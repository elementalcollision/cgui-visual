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
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::model::Container;

/// Sample retention. Seven days at 2 s ticks ≈ 300k rows per container —
/// trivial for SQLite. Adjustable later via prefs if needed.
const RETENTION_SECS: i64 = 7 * 24 * 3600;

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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let conn = open_with_schema(&path).or_else(|_| {
        // If the file is corrupt, blow it away and start fresh — the
        // history is best-effort and not worth a hard failure.
        let _ = std::fs::remove_file(&path);
        open_with_schema(&path)
    })?;
    let _ = DB.set(Mutex::new(conn));
    Ok(())
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
        "#,
    )
    .context("init schema")?;
    Ok(conn)
}

/// Record one tick's worth of samples. Called from the container poll
/// loop — silently no-ops when init() hasn't been called or fails.
pub fn record(snapshot: &[Container]) {
    let Some(db) = DB.get() else { return };
    let now = Utc::now().timestamp();
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
        let cutoff = now - RETENTION_SECS;
        if let Err(e) = conn.execute(
            "DELETE FROM container_history WHERE ts < ?",
            params![cutoff],
        ) {
            eprintln!("history gc: {e}");
        }
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
    let scanned_at = Utc::now().timestamp();
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

/// Return points for a container within the last `since_secs`. Ordered
/// ascending by ts so consumers can stream into a sparkline as-is.
/// Empty when the DB isn't initialised or has no rows for `id`.
pub fn load(id: &str, since_secs: i64) -> Vec<HistoryPoint> {
    let Some(db) = DB.get() else { return vec![] };
    let now = Utc::now().timestamp();
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
}
