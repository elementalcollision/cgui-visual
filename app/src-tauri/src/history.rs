// Long-form container metric history (B6).
//
// Persists per-tick CPU/mem/net/disk samples to a SQLite sidecar at
// ~/.config/cgui-gui/history.db so the inspect modal can render a
// 24h trend chart instead of just the in-memory ring buffer.
//
// Design notes:
//   • One row per container per tick. PRIMARY KEY (id, ts) means the 2 s
//     poll tick is naturally idempotent — two record() calls in the same
//     second are no-ops (we use INSERT OR REPLACE).
//   • GC prunes rows older than RETENTION_SECS on first record() of each
//     calendar day (cheap; runs once per cgui-day, not per poll).
//   • All access funnels through a Mutex<Connection>. With ~20 containers
//     × one INSERT every 2 s, contention is trivial.
//   • Errors surface only on init(); per-tick failures are logged and
//     swallowed so a corrupt DB can't take the live UI down.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::model::Container;

/// Sample retention. Seven days at 2 s ticks ≈ 300k rows per container —
/// trivial for SQLite. Adjustable later via prefs if needed.
const RETENTION_SECS: i64 = 7 * 24 * 3600;
/// Persisted log lines age out on the same schedule as metrics (they are
/// additionally capped per container by LOGS_PER_CONTAINER).
const LOG_RETENTION_SECS: i64 = RETENTION_SECS;
/// Trivy scan summaries are kept for 90 days...
const SCAN_RETENTION_SECS: i64 = 90 * 24 * 3600;
/// ...except that the newest this-many scans per image are always kept,
/// however old: the vuln-trend UI diffs the latest scan against the one
/// before it (`load_scans`), so a single survivor would lose the diff.
const SCANS_KEEP_PER_IMAGE: i64 = 2;

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();
static LAST_GC_DAY: Mutex<Option<i64>> = Mutex::new(None);

fn db_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("cgui-gui").join("history.db"))
}

/// Open the DB and run idempotent schema setup. Safe to call repeatedly;
/// only the first call actually opens. Returns Err only when the file
/// system can't be reached at all — corrupt DBs are recreated.
pub fn init() -> Result<()> {
    if DB.get().is_some() {
        return Ok(());
    }
    let path = db_path().context("could not resolve $XDG_CONFIG_HOME or $HOME")?;
    let conn = open_secure(&path)?;
    let _ = DB.set(Mutex::new(conn));
    Ok(())
}

/// Create the (private) config dir, open the DB with owner-only
/// permissions, and recover from a corrupt file by recreating it.
fn open_secure(path: &std::path::Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        secure_dir(parent)?;
    }
    let open = || -> Result<Connection> {
        // Create the main file 0600 *before* SQLite does, so it is never
        // world-readable; SQLite gives -wal/-shm the main file's mode.
        create_private_file(path)?;
        let conn = open_with_schema(path)?;
        secure_db_files(path);
        Ok(conn)
    };
    open().or_else(|_| {
        // If the file is corrupt, blow it away and start fresh — the
        // history is best-effort and not worth a hard failure. The WAL
        // and SHM sidecars go too: a stale -wal would be replayed
        // against the fresh file.
        remove_db_files(path);
        open()
    })
}

/// Create `dir` (and parents) and force mode 0700 on it.
fn secure_dir(dir: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .with_context(|| format!("create {}", dir.display()))?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("chmod {}", dir.display()))
}

/// Ensure `path` exists with mode 0600 (tightening an existing file).
fn create_private_file(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("create {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("chmod {}", path.display()))
}

fn sidecar(path: &std::path::Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(suffix);
    PathBuf::from(os)
}

/// chmod 0600 the main DB file and any `-wal` / `-shm` that exist.
fn secure_db_files(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    for p in [
        path.to_path_buf(),
        sidecar(path, "-wal"),
        sidecar(path, "-shm"),
    ] {
        if p.exists() {
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
    }
}

/// Delete the main DB file and its `-wal` / `-shm` sidecars.
fn remove_db_files(path: &std::path::Path) {
    for p in [
        path.to_path_buf(),
        sidecar(path, "-wal"),
        sidecar(path, "-shm"),
    ] {
        let _ = std::fs::remove_file(p);
    }
}

fn open_with_schema(path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.pragma_update(None, "synchronous", "NORMAL").ok();
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS container_history (
            id        TEXT    NOT NULL,
            ts        INTEGER NOT NULL,
            cpu       REAL    NOT NULL,
            mem_used  REAL    NOT NULL,
            net_bps   REAL    NOT NULL,
            disk_bps  REAL    NOT NULL,
            status    TEXT    NOT NULL,
            PRIMARY KEY (id, ts)
        );
        CREATE INDEX IF NOT EXISTS idx_history_ts ON container_history (ts);

        -- Image vulnerability scan log (B7). Each row is the summary of a
        -- single trivy run; per-finding CVE detail lives in trivy_findings.
        -- We keep the JSON of cve ids so "new CVEs since last scan" is
        -- a cheap set-diff without a join.
        CREATE TABLE IF NOT EXISTS trivy_scans (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            image       TEXT    NOT NULL,
            scanned_at  INTEGER NOT NULL,
            critical    INTEGER NOT NULL DEFAULT 0,
            high        INTEGER NOT NULL DEFAULT 0,
            medium      INTEGER NOT NULL DEFAULT 0,
            low         INTEGER NOT NULL DEFAULT 0,
            total       INTEGER NOT NULL DEFAULT 0,
            cve_ids     TEXT    NOT NULL DEFAULT '[]'
        );
        CREATE INDEX IF NOT EXISTS idx_trivy_scans_image
            ON trivy_scans (image, scanned_at);

        -- Per-container log persistence (B12). Append-only line buffer.
        -- Trimmed by record_log when row count for a container exceeds
        -- LOGS_PER_CONTAINER. Indexed by (container_id, ts) so loading
        -- the most-recent N lines for one container is a fast tail-scan.
        CREATE TABLE IF NOT EXISTS container_logs (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            container_id  TEXT    NOT NULL,
            ts            INTEGER NOT NULL,
            line          TEXT    NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_logs_container_ts
            ON container_logs (container_id, ts);
        "#,
    )
    .context("init schema")?;
    Ok(conn)
}

/// Cap on persisted log lines per container. Avoids the DB ballooning
/// when a chatty service runs for days. Old lines fall off; live tail
/// is always preserved. 50k lines × ~100 B average ≈ 5 MiB per
/// container, plenty for "what happened five minutes ago" use cases.
const LOGS_PER_CONTAINER: i64 = 50_000;

/// History/log rows are keyed by the FULL container id (`Container.id`).
/// Before the full-id fix they were keyed by an 8-char truncation, so rows
/// written by older builds for containers with ids longer than 8 chars are
/// orphaned under the short key. No migration: the short key cannot be
/// mapped back to one container safely (truncations collide, e.g.
/// `stack_web` / `stack_api`), and aliasing old rows in at read time would
/// show another container's data. Orphaned metric rows are removed by the
/// existing 7-day retention GC; orphaned log rows are simply unreachable
/// (the UI only queries by full id) until the user clears the history DB.
/// Containers whose id is <= 8 chars keep their existing history unchanged.
///
/// Record one tick's worth of samples. Called from the container poll
/// loop — silently no-ops when init() hasn't been called or fails.
pub fn record(snapshot: &[Container]) {
    let Some(db) = DB.get() else { return };
    let now = crate::timeutil::now_unix();
    if let Ok(mut conn) = db.lock() {
        let tx = match conn.transaction() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("history record begin: {e}");
                return;
            }
        };
        for c in snapshot {
            let res = tx.execute(
                "INSERT OR REPLACE INTO container_history
                 (id, ts, cpu, mem_used, net_bps, disk_bps, status)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![
                    c.id,
                    now,
                    c.cpu,
                    c.mem.used,
                    c.net_io_bps,
                    c.disk_io_bps,
                    c.status,
                ],
            );
            if let Err(e) = res {
                eprintln!("history record row {}: {e}", c.id);
            }
        }
        if let Err(e) = tx.commit() {
            eprintln!("history record commit: {e}");
        }
    }
    maybe_gc(now);
}

fn maybe_gc(now: i64) {
    let today = now / 86400;
    let mut last = LAST_GC_DAY.lock().expect("LAST_GC_DAY mutex");
    if last.is_some_and(|d| d == today) {
        return;
    }
    *last = Some(today);
    let Some(db) = DB.get() else { return };
    if let Ok(conn) = db.lock() {
        gc_conn(&conn, now);
    }
}

/// Age-based pruning of every table. Each statement is independent and
/// best-effort so one failure can't block the others.
fn gc_conn(conn: &Connection, now: i64) {
    if let Err(e) = conn.execute(
        "DELETE FROM container_history WHERE ts < ?",
        params![now - RETENTION_SECS],
    ) {
        eprintln!("history gc (metrics): {e}");
    }
    if let Err(e) = conn.execute(
        "DELETE FROM container_logs WHERE ts < ?",
        params![now - LOG_RETENTION_SECS],
    ) {
        eprintln!("history gc (logs): {e}");
    }
    // Old scans go, except the newest SCANS_KEEP_PER_IMAGE of each image.
    if let Err(e) = conn.execute(
        "DELETE FROM trivy_scans
         WHERE scanned_at < ?1
           AND id NOT IN (
               SELECT id FROM (
                   SELECT id, ROW_NUMBER() OVER (
                       PARTITION BY image ORDER BY scanned_at DESC, id DESC
                   ) AS rn
                   FROM trivy_scans
               ) WHERE rn <= ?2
           )",
        params![now - SCAN_RETENTION_SECS, SCANS_KEEP_PER_IMAGE],
    ) {
        eprintln!("history gc (scans): {e}");
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPoint {
    pub ts: i64,
    pub cpu: f64,
    pub mem_used: f64,
    pub net_bps: f64,
    pub disk_bps: f64,
    pub status: String,
}

// ─── Trivy scan history (B7) ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPoint {
    pub scanned_at: i64,
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VulnHistory {
    /// Most-recent-first list of summary points for the trend strip.
    pub points: Vec<ScanPoint>,
    /// CVE ids that appeared in the latest scan but not the previous
    /// one. Empty when there's only one scan (or none) on record.
    pub new_since_last: Vec<String>,
}

/// Persist the result of a single trivy scan. Best-effort — silently
/// no-ops when the DB isn't initialised. Per-severity counts are derived
/// from the finding list; we also store the set of CVE ids so the next
/// "new CVEs since last scan" diff doesn't require re-running trivy.
pub fn record_scan(image: &str, findings: &[crate::model::TrivyFinding]) {
    let Some(db) = DB.get() else { return };
    let scanned_at = crate::timeutil::now_unix();
    let mut critical = 0u32;
    let mut high = 0u32;
    let mut medium = 0u32;
    let mut low = 0u32;
    let mut ids = Vec::with_capacity(findings.len());
    for f in findings {
        match f.sev.as_str() {
            "CRITICAL" => critical += 1,
            "HIGH" => high += 1,
            "MEDIUM" => medium += 1,
            _ => low += 1,
        }
        ids.push(f.cve.clone());
    }
    let total = findings.len() as u32;
    let cve_ids = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());
    if let Ok(conn) = db.lock() {
        let res = conn.execute(
            "INSERT INTO trivy_scans
             (image, scanned_at, critical, high, medium, low, total, cve_ids)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![image, scanned_at, critical, high, medium, low, total, cve_ids],
        );
        if let Err(e) = res {
            eprintln!("trivy_scans record: {e}");
        }
    }
}

/// Load scan history for one image plus the new-CVEs diff vs the
/// previous scan. Empty when no scans are recorded for `image` yet.
pub fn load_scans(image: &str, limit: i64) -> VulnHistory {
    let Some(db) = DB.get() else {
        return VulnHistory {
            points: vec![],
            new_since_last: vec![],
        };
    };
    let conn = match db.lock() {
        Ok(c) => c,
        Err(_) => {
            return VulnHistory {
                points: vec![],
                new_since_last: vec![],
            };
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT scanned_at, critical, high, medium, low, total, cve_ids
         FROM trivy_scans
         WHERE image = ?
         ORDER BY scanned_at DESC
         LIMIT ?",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("trivy_scans prepare: {e}");
            return VulnHistory {
                points: vec![],
                new_since_last: vec![],
            };
        }
    };
    let rows: Vec<(ScanPoint, String)> = stmt
        .query_map(params![image, limit.max(2)], |row| {
            Ok((
                ScanPoint {
                    scanned_at: row.get(0)?,
                    critical: row.get(1)?,
                    high: row.get(2)?,
                    medium: row.get(3)?,
                    low: row.get(4)?,
                    total: row.get(5)?,
                },
                row.get(6)?,
            ))
        })
        .map(|it| it.flatten().collect())
        .unwrap_or_default();

    // Compute the new-CVEs diff: CVEs in the latest scan that weren't
    // in the prior one. Keeps the diff scoped to consecutive scans
    // (regression detection) rather than against the all-time set.
    let mut new_since_last = Vec::new();
    if rows.len() >= 2 {
        let latest: Vec<String> = serde_json::from_str(&rows[0].1).unwrap_or_default();
        let prior: Vec<String> = serde_json::from_str(&rows[1].1).unwrap_or_default();
        let prior_set: std::collections::HashSet<&String> = prior.iter().collect();
        for cve in latest {
            if !prior_set.contains(&cve) {
                new_since_last.push(cve);
            }
        }
    }

    VulnHistory {
        points: rows.into_iter().map(|(p, _)| p).collect(),
        new_since_last,
    }
}

// ─── Per-container log persistence (B12) ──────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub ts: i64,
    pub line: String,
}

/// Flush a batch once it holds this many lines…
const LOG_BATCH_MAX: usize = 200;
/// …or once the oldest queued line has waited this long.
const LOG_BATCH_WINDOW: Duration = Duration::from_millis(100);
/// Rows over the cap tolerated before a trim runs (5% slack so a
/// container sitting at the cap isn't trimmed on every flush).
const LOGS_TRIM_SLACK: i64 = LOGS_PER_CONTAINER / 20;

enum LogMsg {
    Line {
        container_id: String,
        ts: i64,
        line: String,
    },
    /// Flush everything queued so far, then ack.
    Flush(mpsc::Sender<()>),
}

static LOG_TX: OnceLock<mpsc::Sender<LogMsg>> = OnceLock::new();

fn log_sender() -> Option<&'static mpsc::Sender<LogMsg>> {
    if let Some(tx) = LOG_TX.get() {
        return Some(tx);
    }
    let (tx, rx) = mpsc::channel();
    // Losing the init race just drops our (unused) channel pair.
    if LOG_TX.set(tx).is_err() {
        return LOG_TX.get();
    }
    let spawned = std::thread::Builder::new()
        .name("cgui-log-writer".into())
        .spawn(move || log_writer_loop(rx));
    if let Err(e) = spawned {
        eprintln!("logs writer thread: {e}");
    }
    LOG_TX.get()
}

/// Queue a log line for persistence. Best-effort and non-blocking —
/// silently no-ops when the DB isn't initialised. A single writer
/// thread batches lines (≤ LOG_BATCH_MAX or LOG_BATCH_WINDOW) into one
/// transaction, so a chatty container no longer takes the global DB
/// mutex per line and can't starve the 2 s metrics `record`.
pub fn record_log(container_id: &str, line: &str) {
    if DB.get().is_none() {
        return;
    }
    if let Some(tx) = log_sender() {
        let _ = tx.send(LogMsg::Line {
            container_id: container_id.to_string(),
            ts: crate::timeutil::now_unix(),
            line: line.to_string(),
        });
    }
}

/// Block (up to 2 s) until every line queued before this call is on
/// disk. Intended for app exit; harmless to call at any time.
pub fn flush_logs() {
    let Some(tx) = LOG_TX.get() else { return };
    let (ack_tx, ack_rx) = mpsc::channel();
    if tx.send(LogMsg::Flush(ack_tx)).is_ok() {
        let _ = ack_rx.recv_timeout(Duration::from_secs(2));
    }
}

fn log_writer_loop(rx: mpsc::Receiver<LogMsg>) {
    // Approximate per-container row counts, seeded lazily from the DB.
    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut batch: Vec<(String, i64, String)> = Vec::new();
    let mut acks: Vec<mpsc::Sender<()>> = Vec::new();
    loop {
        // Wait for the first message, then gather more until the window
        // closes or the batch is full.
        let first = match rx.recv() {
            Ok(m) => m,
            Err(_) => return,
        };
        let mut disconnected = false;
        let deadline = Instant::now() + LOG_BATCH_WINDOW;
        let mut next = Some(first);
        while let Some(msg) = next.take() {
            match msg {
                LogMsg::Line {
                    container_id,
                    ts,
                    line,
                } => batch.push((container_id, ts, line)),
                LogMsg::Flush(ack) => {
                    acks.push(ack);
                    break;
                }
            }
            if batch.len() >= LOG_BATCH_MAX {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(remaining) {
                Ok(m) => next = Some(m),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => disconnected = true,
            }
        }
        if !batch.is_empty() {
            if let Some(db) = DB.get() {
                if let Ok(mut conn) = db.lock() {
                    if let Err(e) = write_log_batch(&mut conn, &batch, &mut counts) {
                        eprintln!("logs record: {e}");
                    }
                }
            }
            batch.clear();
        }
        for ack in acks.drain(..) {
            let _ = ack.send(());
        }
        if disconnected {
            return;
        }
    }
}

/// Insert `batch` in one transaction, then trim any container whose
/// (approximate, in-memory) row count exceeds cap + slack. Counts are
/// seeded with one COUNT(*) the first time a container is seen, so the
/// steady state is a plain batched INSERT with no per-line query.
fn write_log_batch(
    conn: &mut Connection,
    batch: &[(String, i64, String)],
    counts: &mut HashMap<String, i64>,
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    let mut touched: Vec<&str> = Vec::new();
    {
        let mut ins = tx.prepare_cached(
            "INSERT INTO container_logs (container_id, ts, line) VALUES (?, ?, ?)",
        )?;
        for (cid, ts, line) in batch {
            if !counts.contains_key(cid) {
                let n: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM container_logs WHERE container_id = ?",
                    params![cid],
                    |r| r.get(0),
                )?;
                counts.insert(cid.clone(), n);
            }
            ins.execute(params![cid, ts, line])?;
            if let Some(c) = counts.get_mut(cid) {
                *c += 1;
            }
            if !touched.contains(&cid.as_str()) {
                touched.push(cid);
            }
        }
    }
    for cid in touched {
        if counts.get(cid).copied().unwrap_or(0) > LOGS_PER_CONTAINER + LOGS_TRIM_SLACK {
            tx.execute(
                "DELETE FROM container_logs
                 WHERE container_id = ?1
                   AND id NOT IN (
                       SELECT id FROM container_logs
                       WHERE container_id = ?1
                       ORDER BY id DESC
                       LIMIT ?2
                   )",
                params![cid, LOGS_PER_CONTAINER],
            )?;
            counts.insert(cid.to_string(), LOGS_PER_CONTAINER);
        }
    }
    tx.commit()
}

/// Load the most recent `limit` lines for one container, optionally
/// filtered by a substring `query`. Returned in ascending-by-ts order
/// so the LogsView can append them to its render buffer as if they
/// arrived live.
pub fn load_logs(container_id: &str, limit: i64, query: Option<&str>) -> Vec<LogLine> {
    let Some(db) = DB.get() else { return vec![] };
    let conn = match db.lock() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let limit = limit.clamp(1, 5000);
    // Pull most-recent N then reverse — much cheaper than ORDER BY ASC
    // with a tail offset, since the (container_id, ts) index is descending-
    // friendly via id DESC LIMIT.
    let mut rows: Vec<LogLine> = match query {
        Some(q) if !q.is_empty() => {
            let pat = format!("%{}%", q);
            let mut stmt = match conn.prepare(
                "SELECT ts, line FROM container_logs
                 WHERE container_id = ? AND line LIKE ?
                 ORDER BY id DESC LIMIT ?",
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("logs load prepare (query): {e}");
                    return vec![];
                }
            };
            stmt.query_map(params![container_id, pat, limit], |r| {
                Ok(LogLine {
                    ts: r.get(0)?,
                    line: r.get(1)?,
                })
            })
            .map(|it| it.flatten().collect())
            .unwrap_or_default()
        }
        _ => {
            let mut stmt = match conn.prepare(
                "SELECT ts, line FROM container_logs
                 WHERE container_id = ?
                 ORDER BY id DESC LIMIT ?",
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("logs load prepare: {e}");
                    return vec![];
                }
            };
            stmt.query_map(params![container_id, limit], |r| {
                Ok(LogLine {
                    ts: r.get(0)?,
                    line: r.get(1)?,
                })
            })
            .map(|it| it.flatten().collect())
            .unwrap_or_default()
        }
    };
    rows.reverse();
    rows
}

/// Return points for a container within the last `since_secs`. Ordered
/// ascending by ts so consumers can stream into a sparkline as-is.
/// Empty when the DB isn't initialised or has no rows for `id`.
pub fn load(id: &str, since_secs: i64) -> Vec<HistoryPoint> {
    let Some(db) = DB.get() else { return vec![] };
    let now = crate::timeutil::now_unix();
    let from = now - since_secs.max(0);
    let conn = match db.lock() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut stmt = match conn.prepare(
        "SELECT ts, cpu, mem_used, net_bps, disk_bps, status
         FROM container_history
         WHERE id = ? AND ts >= ?
         ORDER BY ts ASC",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("history load prepare: {e}");
            return vec![];
        }
    };
    let rows = stmt.query_map(params![id, from], |row| {
        Ok(HistoryPoint {
            ts: row.get(0)?,
            cpu: row.get(1)?,
            mem_used: row.get(2)?,
            net_bps: row.get(3)?,
            disk_bps: row.get(4)?,
            status: row.get(5)?,
        })
    });
    match rows {
        Ok(it) => it.flatten().collect(),
        Err(e) => {
            eprintln!("history load query: {e}");
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Container, MemUsage};

    fn mk_container(id: &str, cpu: f64) -> Container {
        Container {
            id: id.into(),
            short_id: id.chars().take(8).collect(),
            name: id.into(),
            image: "test".into(),
            status: "running".into(),
            cpu,
            mem: MemUsage {
                used: 1.0,
                limit: 8.0,
                unit: "GiB".into(),
                pct: 12.5,
            },
            net_io_bps: 100.0,
            disk_io_bps: 50.0,
            ..Default::default()
        }
    }

    // Tests can't share the OnceLock without leaking state across runs —
    // exercise the schema/insert/query pipeline against an isolated
    // in-memory connection instead so each test is hermetic.
    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE container_history (
                id TEXT NOT NULL, ts INTEGER NOT NULL, cpu REAL NOT NULL,
                mem_used REAL NOT NULL, net_bps REAL NOT NULL,
                disk_bps REAL NOT NULL, status TEXT NOT NULL,
                PRIMARY KEY (id, ts)
            );
            "#,
        )
        .unwrap();
        conn
    }

    fn logs_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE container_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                container_id TEXT NOT NULL, ts INTEGER NOT NULL, line TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn lines_for(conn: &Connection, cid: &str) -> Vec<String> {
        let mut st = conn
            .prepare("SELECT line FROM container_logs WHERE container_id = ? ORDER BY id")
            .unwrap();
        st.query_map([cid], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    fn batch_of(cid: &str, from: usize, n: usize) -> Vec<(String, i64, String)> {
        (from..from + n)
            .map(|i| (cid.to_string(), 1, format!("l{i}")))
            .collect()
    }

    #[test]
    fn log_batch_inserts_in_order_and_counts() {
        let mut conn = logs_conn();
        let mut counts = HashMap::new();
        let mut b = batch_of("a", 0, 3);
        b.extend(batch_of("b", 0, 2));
        write_log_batch(&mut conn, &b, &mut counts).unwrap();
        assert_eq!(lines_for(&conn, "a"), vec!["l0", "l1", "l2"]);
        assert_eq!(lines_for(&conn, "b"), vec!["l0", "l1"]);
        assert_eq!(counts["a"], 3);
        assert_eq!(counts["b"], 2);
    }

    #[test]
    fn log_batch_seeds_count_from_existing_rows() {
        let mut conn = logs_conn();
        write_log_batch(&mut conn, &batch_of("a", 0, 5), &mut HashMap::new()).unwrap();
        // Fresh in-memory counter (e.g. after restart) re-seeds from the DB.
        let mut counts = HashMap::new();
        write_log_batch(&mut conn, &batch_of("a", 5, 2), &mut counts).unwrap();
        assert_eq!(counts["a"], 7);
    }

    #[test]
    fn log_batch_trims_only_past_slack_and_only_that_container() {
        let mut conn = logs_conn();
        let mut counts = HashMap::new();
        write_log_batch(&mut conn, &batch_of("other", 0, 10), &mut counts).unwrap();
        // Exactly cap + slack: no trim yet.
        let at_limit = (LOGS_PER_CONTAINER + LOGS_TRIM_SLACK) as usize;
        write_log_batch(&mut conn, &batch_of("a", 0, at_limit), &mut counts).unwrap();
        assert_eq!(counts["a"], LOGS_PER_CONTAINER + LOGS_TRIM_SLACK);
        // One more line crosses the threshold: trimmed back to the cap,
        // keeping the newest rows.
        write_log_batch(&mut conn, &batch_of("a", at_limit, 1), &mut counts).unwrap();
        assert_eq!(counts["a"], LOGS_PER_CONTAINER);
        let kept = lines_for(&conn, "a");
        assert_eq!(kept.len() as i64, LOGS_PER_CONTAINER);
        assert_eq!(kept.last().unwrap(), &format!("l{at_limit}"));
        assert_eq!(lines_for(&conn, "other").len(), 10);
    }

    #[test]
    fn schema_compiles_and_inserts() {
        let conn = fresh_conn();
        let c = mk_container("abc", 12.5);
        conn.execute(
            "INSERT INTO container_history VALUES (?,?,?,?,?,?,?)",
            params![
                c.id,
                100,
                c.cpu,
                c.mem.used,
                c.net_io_bps,
                c.disk_io_bps,
                c.status
            ],
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM container_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn duplicate_ts_replaces_not_inserts() {
        let conn = fresh_conn();
        for cpu in [10.0, 22.0, 35.0] {
            conn.execute(
                "INSERT OR REPLACE INTO container_history VALUES (?,?,?,?,?,?,?)",
                params!["xyz", 200, cpu, 1.0, 0.0, 0.0, "running"],
            )
            .unwrap();
        }
        let cpu: f64 = conn
            .query_row(
                "SELECT cpu FROM container_history WHERE id = 'xyz' AND ts = 200",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cpu, 35.0);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM container_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn trivy_scan_diff_finds_new_cves() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE trivy_scans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                image TEXT NOT NULL,
                scanned_at INTEGER NOT NULL,
                critical INTEGER NOT NULL DEFAULT 0,
                high INTEGER NOT NULL DEFAULT 0,
                medium INTEGER NOT NULL DEFAULT 0,
                low INTEGER NOT NULL DEFAULT 0,
                total INTEGER NOT NULL DEFAULT 0,
                cve_ids TEXT NOT NULL DEFAULT '[]'
            );
            "#,
        )
        .unwrap();

        // Two scans for the same image, second introduces CVE-NEW-1.
        let prior = r#"["CVE-A","CVE-B"]"#;
        let latest = r#"["CVE-A","CVE-B","CVE-NEW-1"]"#;
        conn.execute(
            "INSERT INTO trivy_scans
             (image, scanned_at, critical, high, medium, low, total, cve_ids)
             VALUES ('alpine:3', 1000, 0, 1, 1, 0, 2, ?)",
            params![prior],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO trivy_scans
             (image, scanned_at, critical, high, medium, low, total, cve_ids)
             VALUES ('alpine:3', 2000, 1, 1, 1, 0, 3, ?)",
            params![latest],
        )
        .unwrap();

        // Mimic the diff logic from load_scans.
        let mut stmt = conn
            .prepare(
                "SELECT cve_ids FROM trivy_scans WHERE image = 'alpine:3'
                 ORDER BY scanned_at DESC LIMIT 2",
            )
            .unwrap();
        let rows: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(rows.len(), 2);
        let latest: Vec<String> = serde_json::from_str(&rows[0]).unwrap();
        let prior: Vec<String> = serde_json::from_str(&rows[1]).unwrap();
        let prior_set: std::collections::HashSet<&String> = prior.iter().collect();
        let new_cves: Vec<&String> = latest.iter().filter(|c| !prior_set.contains(c)).collect();
        assert_eq!(new_cves.len(), 1);
        assert_eq!(new_cves[0], "CVE-NEW-1");
    }

    #[test]
    fn log_query_returns_most_recent_first_then_chronological() {
        // Mirrors the DESC-then-reverse strategy used by load_logs.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE container_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                container_id TEXT NOT NULL,
                ts INTEGER NOT NULL,
                line TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        for (i, line) in ["a", "b", "c", "d", "e"].iter().enumerate() {
            conn.execute(
                "INSERT INTO container_logs (container_id, ts, line) VALUES (?, ?, ?)",
                params!["x", 100 + i as i64, line],
            )
            .unwrap();
        }
        let mut stmt = conn
            .prepare(
                "SELECT line FROM container_logs WHERE container_id = ?
                 ORDER BY id DESC LIMIT ?",
            )
            .unwrap();
        let mut rows: Vec<String> = stmt
            .query_map(params!["x", 3], |r| r.get(0))
            .unwrap()
            .flatten()
            .collect();
        rows.reverse();
        assert_eq!(rows, vec!["c", "d", "e"]);
    }

    #[test]
    fn time_window_query_filters_by_id_and_ts() {
        let conn = fresh_conn();
        for ts in [100, 200, 300, 400] {
            for id in ["a", "b"] {
                conn.execute(
                    "INSERT INTO container_history VALUES (?,?,?,?,?,?,?)",
                    params![id, ts, 1.0, 0.0, 0.0, 0.0, "running"],
                )
                .unwrap();
            }
        }
        let mut stmt = conn
            .prepare("SELECT ts FROM container_history WHERE id = ? AND ts >= ? ORDER BY ts ASC")
            .unwrap();
        let got: Vec<i64> = stmt
            .query_map(params!["a", 250], |r| r.get(0))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(got, vec![300, 400]);
    }

    // ── Permissions, recovery and retention (real schema, temp dir) ──

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cgui-history-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn mode_of(p: &std::path::Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn open_secure_makes_dir_0700_and_files_0600() {
        let dir = scratch("perm").join("cgui-gui");
        let db = dir.join("history.db");
        let conn = open_secure(&db).unwrap();
        // Force the WAL/SHM sidecars into existence.
        conn.execute(
            "INSERT INTO container_logs (container_id, ts, line) VALUES ('a', 1, 'x')",
            [],
        )
        .unwrap();
        secure_db_files(&db);
        assert_eq!(mode_of(&dir), 0o700);
        assert_eq!(mode_of(&db), 0o600);
        for suffix in ["-wal", "-shm"] {
            let p = sidecar(&db, suffix);
            if p.exists() {
                assert_eq!(mode_of(&p), 0o600, "{suffix}");
            }
        }
        drop(conn);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn open_secure_tightens_existing_loose_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("tighten").join("cgui-gui");
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("history.db");
        std::fs::write(&db, b"").unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o644)).unwrap();
        let conn = open_secure(&db).unwrap();
        assert_eq!(mode_of(&dir), 0o700);
        assert_eq!(mode_of(&db), 0o600);
        drop(conn);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn corrupt_db_recovery_removes_wal_and_shm_too() {
        let dir = scratch("corrupt").join("cgui-gui");
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("history.db");
        std::fs::write(
            &db,
            b"this is not a sqlite database, just garbage bytes....",
        )
        .unwrap();
        std::fs::write(sidecar(&db, "-wal"), b"stale wal").unwrap();
        std::fs::write(sidecar(&db, "-shm"), b"stale shm").unwrap();
        let conn = open_secure(&db).unwrap();
        // Recreated and usable; the stale sidecar contents are gone.
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM container_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
        for suffix in ["-wal", "-shm"] {
            let p = sidecar(&db, suffix);
            if p.exists() {
                assert_ne!(
                    std::fs::read(&p).unwrap(),
                    format!("stale {}", &suffix[1..]).as_bytes()
                );
            }
        }
        drop(conn);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn remove_db_files_deletes_all_three() {
        let dir = scratch("rm");
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("history.db");
        for p in [db.clone(), sidecar(&db, "-wal"), sidecar(&db, "-shm")] {
            std::fs::write(p, b"x").unwrap();
        }
        remove_db_files(&db);
        assert!(!db.exists() && !sidecar(&db, "-wal").exists() && !sidecar(&db, "-shm").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn gc_db(tag: &str) -> Connection {
        let dir = scratch(tag);
        open_secure(&dir.join("cgui-gui").join("history.db")).unwrap()
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn gc_prunes_metrics_and_logs_by_age() {
        let conn = gc_db("gc-logs");
        let now = 10_000_000_i64;
        let old = now - RETENTION_SECS - 10;
        let fresh = now - 60;
        for ts in [old, fresh] {
            conn.execute(
                "INSERT INTO container_history VALUES ('a', ?, 1, 0, 0, 0, 'running')",
                params![ts],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO container_logs (container_id, ts, line) VALUES ('a', ?, 'l')",
                params![ts],
            )
            .unwrap();
        }
        gc_conn(&conn, now);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM container_history"), 1);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM container_logs"), 1);
        assert_eq!(
            count(&conn, "SELECT ts FROM container_logs"),
            fresh,
            "the recent line survives"
        );
    }

    #[test]
    fn gc_scans_keeps_recent_and_newest_two_per_image() {
        let conn = gc_db("gc-scans-keep");
        let now = 100_000_000_i64;
        let day = 86_400;
        let ins = |image: &str, age_days: i64| {
            conn.execute(
                "INSERT INTO trivy_scans (image, scanned_at, total) VALUES (?, ?, 1)",
                params![image, now - age_days * day],
            )
            .unwrap();
        };
        // `old`: four ancient scans -> only the newest two (200d, 150d) stay.
        for d in [400, 300, 200, 150] {
            ins("old", d);
        }
        // `mixed`: one fresh, two ancient -> fresh + newest ancient survive
        // (newest two overall), the oldest ancient goes.
        for d in [10, 300, 500] {
            ins("mixed", d);
        }
        // `lone`: single ancient scan is never pruned.
        ins("lone", 1000);
        gc_conn(&conn, now);
        let ages = |image: &str| -> Vec<i64> {
            let mut st = conn
                .prepare(
                    "SELECT scanned_at FROM trivy_scans WHERE image = ? ORDER BY scanned_at DESC",
                )
                .unwrap();
            st.query_map(params![image], |r| r.get::<_, i64>(0))
                .unwrap()
                .flatten()
                .map(|t| (now - t) / day)
                .collect()
        };
        assert_eq!(ages("old"), vec![150, 200]);
        assert_eq!(ages("mixed"), vec![10, 300]);
        assert_eq!(ages("lone"), vec![1000]);
    }

    #[test]
    fn gc_scans_drops_old_scans_beyond_the_newest_two_but_keeps_all_recent() {
        let conn = gc_db("gc-scans-recent");
        let now = 100_000_000_i64;
        for d in [1, 2, 3, 4] {
            conn.execute(
                "INSERT INTO trivy_scans (image, scanned_at, total) VALUES ('img', ?, 1)",
                params![now - d * 86_400],
            )
            .unwrap();
        }
        gc_conn(&conn, now);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM trivy_scans"), 4);
    }
}
