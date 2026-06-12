// Thin async wrapper around the active container runtime CLI (defaults to
// Apple's `container`). Mirrors the parsing logic in cgui's container.rs so
// the rest of the app gets typed `Container` values regardless of which CLI
// is active.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::process::Stdio;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::model::{Container, Image, MemUsage, Network, Volume};

// Active runtime binary, settable from prefs at startup and from the Settings
// modal. `bin()` returns a snapshot for the call site.
static RUNTIME_BIN: OnceLock<RwLock<String>> = OnceLock::new();

fn slot() -> &'static RwLock<String> {
    RUNTIME_BIN.get_or_init(|| RwLock::new("container".to_string()))
}

pub fn set_bin(name: &str) {
    *slot().write().unwrap() = name.to_string();
}

/// Prepend the standard macOS install locations to our process's PATH
/// so spawned children can resolve `container` / `docker` / `podman` /
/// `trivy` regardless of how the app was launched.
///
/// **Why this is needed:** when a Tauri app is launched from Finder,
/// Spotlight, the Dock, or Launch Services, child processes inherit
/// the launchd PATH (`/usr/bin:/bin:/usr/sbin:/sbin`) — *not* the
/// user's interactive shell PATH. Apple's `container` installer drops
/// the binary at `/usr/local/bin/container`, and Homebrew on Apple
/// Silicon installs to `/opt/homebrew/bin`; both of those are absent
/// from the inherited PATH, so `Command::new("container")` fails with
/// "No such file or directory" even though `which container` works
/// fine in Terminal.
///
/// We deduplicate against existing PATH entries so this is idempotent
/// and safe to call from `setup()`. Called once at boot from lib.rs.
pub fn ensure_user_path() {
    let current = std::env::var("PATH").unwrap_or_default();
    if let Some(patched) = patch_path(&current, COMMON_BIN_DIRS) {
        // Safe: this runs single-threaded during `setup()`, before any
        // tokio worker spawns or pty thread launches.
        std::env::set_var("PATH", patched);
    }
}

const COMMON_BIN_DIRS: &[&str] = &[
    "/usr/local/bin",
    "/usr/local/sbin",
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
];

/// Pure helper for `ensure_user_path` so it can be unit-tested without
/// touching process-global env state. Returns the new PATH when at
/// least one entry needs prepending; returns None when every needed
/// dir is already present (so the caller can skip the env mutation).
fn patch_path(current: &str, needed: &[&str]) -> Option<String> {
    let existing: std::collections::HashSet<&str> =
        current.split(':').filter(|s| !s.is_empty()).collect();
    let prepend: Vec<&str> = needed
        .iter()
        .copied()
        .filter(|p| !existing.contains(p))
        .collect();
    if prepend.is_empty() {
        return None;
    }
    Some(if current.is_empty() {
        prepend.join(":")
    } else {
        format!("{}:{}", prepend.join(":"), current)
    })
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct StatRow {
    pub id: String,
    // Apple's `container stats` reports cumulative CPU time in microseconds.
    // CPU% is computed in state.rs via deltas across polls.
    #[serde(rename = "cpuUsageUsec", alias = "cpu_usage_usec")]
    pub cpu_usage_usec: u64,
    #[serde(
        rename = "memoryUsageBytes",
        alias = "memory_usage_bytes",
        alias = "memoryUsage"
    )]
    pub memory_usage_bytes: u64,
    #[serde(
        rename = "memoryLimitBytes",
        alias = "memory_limit_bytes",
        alias = "memoryLimit"
    )]
    pub memory_limit_bytes: u64,
    #[serde(rename = "networkRxBytes", alias = "network_rx_bytes", default)]
    pub network_rx_bytes: u64,
    #[serde(rename = "networkTxBytes", alias = "network_tx_bytes", default)]
    pub network_tx_bytes: u64,
    #[serde(rename = "blockReadBytes", alias = "block_read_bytes", default)]
    pub block_read_bytes: u64,
    #[serde(rename = "blockWriteBytes", alias = "block_write_bytes", default)]
    pub block_write_bytes: u64,
}

const RUN_TIMEOUT: Duration = Duration::from_secs(8);

/// Timeout for `container run` invocations. Even with `-d` (detach),
/// Apple's `container` may need to pull a missing image and stand up
/// the lightweight VM, which is comfortably out of the 8 s envelope
/// the rest of `run` uses for fast inspect-style queries. 3 minutes
/// is generous enough for a first-pull of a small base image like
/// alpine on a slow connection.
const RUN_LONG_TIMEOUT: Duration = Duration::from_secs(180);

fn bin() -> String {
    slot().read().unwrap().clone()
}

/// Public accessor for the currently-active runtime binary name. Used
/// by callers (e.g. pty.rs) that want to compose their own argv without
/// going through the `run`/`spawn` helpers.
pub fn current_bin() -> String {
    bin()
}

// True when the binary is reachable on PATH. Used so the UI can fall back to
// fixtures on machines without Apple's container runtime installed.
pub async fn available() -> bool {
    Command::new(bin())
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Per-runtime availability probe (B8). Doesn't read or mutate the
// active-runtime slot; lets the Settings UI surface availability badges
// for each candidate without flipping the active selection.
pub async fn probe_bin(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

async fn run(args: &[&str]) -> Result<Vec<u8>> {
    run_with_timeout(args, RUN_TIMEOUT).await
}

/// Variant of `run` with a caller-chosen timeout for operations that
/// are inherently slower than fast inspect / list queries (image pull,
/// `container run`, etc.). Same error envelope as `run`.
async fn run_with_timeout(args: &[&str], timeout: Duration) -> Result<Vec<u8>> {
    let fut = Command::new(bin()).args(args).output();
    let out = tokio::time::timeout(timeout, fut)
        .await
        .map_err(|_| {
            anyhow!(
                "`{} {}` timed out after {}s",
                bin(),
                args.join(" "),
                timeout.as_secs()
            )
        })?
        .with_context(|| format!("failed to spawn `{} {}`", bin(), args.join(" ")))?;
    if !out.status.success() {
        return Err(anyhow!(
            "`{} {}` exited {}: {}",
            bin(),
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

// Raw ls + stats; the merge happens in state.rs because CPU% needs deltas
// across polls, which require persistent state.
pub async fn list_containers_raw() -> Result<(Vec<Container>, Vec<StatRow>)> {
    let (ls_res, stats_res) =
        tokio::join!(run(&["ls", "--all", "--format", "json"]), stats_snapshot(),);
    let bytes = ls_res?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).context("parse `container ls` json")?;
    let cs: Vec<Container> = raw.into_iter().map(parse_container).collect();
    Ok((cs, stats_res.unwrap_or_default()))
}

pub async fn stats_snapshot() -> Result<Vec<StatRow>> {
    let bytes = run(&["stats", "--no-stream", "--format", "json"]).await?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    Ok(raw
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

pub fn match_stat<'a>(stats: &'a [StatRow], short_id: &str) -> Option<&'a StatRow> {
    stats
        .iter()
        .find(|s| s.id.starts_with(short_id) || short_id.starts_with(&s.id))
}

fn parse_container(v: Value) -> Container {
    let cfg = v.get("configuration").cloned().unwrap_or(Value::Null);
    let id = cfg
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let image = cfg
        .get("image")
        .and_then(|i| i.get("reference"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    // 0.x reported `status` as a plain string; 1.0 nests it as
    // `{"state": "running", "startedDate": "...", "networks": [...]}`.
    let status = v
        .get("status")
        .and_then(|s| {
            s.as_str()
                .or_else(|| s.get("state").and_then(Value::as_str))
        })
        .unwrap_or("unknown")
        .to_string();
    let memory_bytes = cfg
        .get("resources")
        .and_then(|r| r.get("memoryInBytes"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let ports = cfg
        .get("publishedPorts")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|p| {
                    let host = p.get("hostPort").and_then(Value::as_u64).unwrap_or(0);
                    let cont = p.get("containerPort").and_then(Value::as_u64).unwrap_or(0);
                    format!("{host}:{cont}")
                })
                .collect()
        })
        .unwrap_or_default();

    let mem_limit_gib = bytes_to_gib(memory_bytes);
    let stack = compose_project(&cfg);

    // Short-id (first 8 chars) for parity with the prototype's display.
    let short_id = id.chars().take(8).collect::<String>();

    // 0.x reported `startedDate` (or `createdDate`) as a top-level float
    // in NSDate reference epoch (seconds since 2001-01-01 UTC). 1.0 moved
    // it to `status.startedDate` as an RFC3339 string, with the creation
    // time at `configuration.creationDate`.
    let started_apple = v
        .get("startedDate")
        .or_else(|| v.get("createdDate"))
        .or_else(|| cfg.get("createdAt"))
        .and_then(Value::as_f64);
    let started_iso = v
        .get("status")
        .and_then(|s| s.get("startedDate"))
        .or_else(|| cfg.get("createdAt"))
        .or_else(|| cfg.get("creationDate"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let started_unix = started_apple.map(apple_epoch_to_unix);

    let normalized = normalize_status(&status);
    let uptime = if normalized == "running" {
        match started_unix {
            Some(t) => format_uptime_from_unix(t),
            None if !started_iso.is_empty() => format_uptime_from_iso(&started_iso),
            _ => "—".into(),
        }
    } else {
        "—".into()
    };

    // Hostname lived at `networks[0].hostname` in 0.x; 1.0 nests it
    // under `networks[0].options.hostname`.
    let name = cfg
        .get("hostname")
        .and_then(Value::as_str)
        .or_else(|| {
            let n0 = cfg.get("networks").and_then(|n| n.get(0));
            n0.and_then(|n| n.get("hostname"))
                .or_else(|| {
                    n0.and_then(|n| n.get("options"))
                        .and_then(|o| o.get("hostname"))
                })
                .and_then(Value::as_str)
        })
        .unwrap_or(&id)
        .to_string();

    let cmd = cfg
        .get("initProcess")
        .or_else(|| cfg.get("processConfig"))
        .map(|p| {
            let exe = p.get("executable").and_then(Value::as_str).unwrap_or("");
            let args = p
                .get("arguments")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut out = Vec::with_capacity(1 + args.len());
            if !exe.is_empty() {
                out.push(exe.to_string());
            }
            out.extend(args);
            out
        })
        .unwrap_or_default();

    Container {
        id: short_id,
        name,
        image,
        status: normalized,
        uptime,
        exit_code: None,
        cpu: 0.0,
        mem: MemUsage {
            used: 0.0,
            limit: mem_limit_gib,
            unit: "GiB".into(),
            pct: 0.0,
        },
        ports,
        stack,
        created: started_iso,
        cpu_history: vec![0.0; 24],
        cmd,
        net_io_bps: 0.0,
        disk_io_bps: 0.0,
        started_unix,
    }
}

// Apple NSDate reference epoch is 2001-01-01 00:00:00 UTC.
const APPLE_EPOCH_OFFSET: f64 = 978_307_200.0;

fn apple_epoch_to_unix(apple_ts: f64) -> i64 {
    (apple_ts + APPLE_EPOCH_OFFSET) as i64
}

fn format_uptime_from_unix(unix_secs: i64) -> String {
    let secs = (Utc::now().timestamp() - unix_secs).max(0);
    format_duration(secs)
}

fn format_uptime_from_iso(s: &str) -> String {
    let Ok(t) = DateTime::parse_from_rfc3339(s) else {
        return "—".into();
    };
    let secs = (Utc::now() - t.with_timezone(&Utc)).num_seconds().max(0);
    format_duration(secs)
}

fn format_duration(secs: i64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d {h:02}h")
    } else if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

fn bytes_to_gib(bytes: u64) -> f64 {
    (bytes as f64) / (1024.0 * 1024.0 * 1024.0)
}

// `container` returns lowercase status strings already; normalize a few
// docker-flavored variants for safety so the UI's status-pill switch hits.
fn normalize_status(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "up" | "running" => "running".into(),
        "paused" => "paused".into(),
        "exited" | "stopped" | "created" => "exited".into(),
        other => other.into(),
    }
}

// Compose project label, mirrors what `cgui` reads for stack grouping.
fn compose_project(cfg: &Value) -> Option<String> {
    cfg.get("labels")
        .and_then(Value::as_object)
        .and_then(|m| m.get("com.docker.compose.project"))
        .and_then(Value::as_str)
        .map(String::from)
}

// ─── Images / Volumes / Networks ──────────────────────────────────────

pub async fn list_images() -> Result<Vec<Image>> {
    let bytes = run(&["image", "ls", "--format", "json"]).await?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).context("parse `image ls` json")?;
    Ok(raw.into_iter().map(parse_image).collect())
}

fn parse_image(v: Value) -> Image {
    // 1.0 nests the metadata under `configuration` (`name`, `creationDate`,
    // `descriptor`) with per-platform byte sizes in `variants[].size`;
    // 0.x had `reference`, `createdAt`, `descriptor`, and a human-formatted
    // `fullSize` string at the top level.
    let cfg = v.get("configuration");
    let reference = cfg
        .and_then(|c| c.get("name"))
        .or_else(|| v.get("reference"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let size = match cfg {
        Some(_) => {
            let bytes: u64 = v
                .get("variants")
                .and_then(Value::as_array)
                .map(|vs| {
                    vs.iter()
                        .filter_map(|p| p.get("size").and_then(Value::as_u64))
                        .sum()
                })
                .unwrap_or(0);
            bytes_to_gib(bytes)
        }
        None => parse_size_to_gib(v.get("fullSize").and_then(Value::as_str).unwrap_or("0")),
    };
    let digest = cfg
        .unwrap_or(&v)
        .get("descriptor")
        .and_then(|d| d.get("digest"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let id = digest
        .split(':')
        .nth(1)
        .map(|h| format!("sha256:{}", &h[..h.len().min(8)]))
        .unwrap_or_else(|| digest.clone());
    Image {
        id,
        reference,
        size,
        size_unit: "GiB".into(),
        created: cfg
            .and_then(|c| c.get("creationDate"))
            .or_else(|| v.get("createdAt"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        tags: v
            .get("tags")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        digest,
        // `image ls` doesn't expose layer count directly; surface 0 for
        // now (a follow-up can call `image inspect` per row to get it).
        layers: 0,
    }
}

// "1.4 GB" / "142 MB" / "8192 bytes" → GiB.
fn parse_size_to_gib(s: &str) -> f64 {
    let s = s.trim();
    let (num_str, unit) = s.split_at(
        s.find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(s.len()),
    );
    let n: f64 = num_str.parse().unwrap_or(0.0);
    let u = unit.trim().to_ascii_uppercase();
    match u.as_str() {
        "B" | "BYTES" => n / (1024.0 * 1024.0 * 1024.0),
        "KB" | "KIB" => n / (1024.0 * 1024.0),
        "MB" | "MIB" => n / 1024.0,
        "GB" | "GIB" => n,
        "TB" | "TIB" => n * 1024.0,
        _ => n / (1024.0 * 1024.0 * 1024.0),
    }
}

pub async fn list_volumes() -> Result<Vec<Volume>> {
    // Fetch the volume list and the container list in parallel — we need the
    // latter to count how many running containers reference each volume.
    let (vol_res, ctr_res) = tokio::join!(
        run(&["volume", "ls", "--format", "json"]),
        run(&["ls", "--all", "--format", "json"]),
    );
    let bytes = vol_res?;
    if bytes.iter().all(|b| b.is_ascii_whitespace()) {
        return Ok(vec![]);
    }
    let raw: Vec<Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    let refs = ctr_res.map(|b| volume_ref_counts(&b)).unwrap_or_default();

    Ok(raw.into_iter().map(|v| parse_volume(v, &refs)).collect())
}

fn parse_volume(v: Value, refs: &std::collections::HashMap<String, u32>) -> Volume {
    // 1.0 nests `name`/`driver`/`source`/`sizeInBytes` under
    // `configuration`; 0.x had them at the top level. Field names are
    // unchanged, so resolve the container object once and read through it.
    let cfg = v.get("configuration").unwrap_or(&v);
    let name = cfg
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let driver = cfg
        .get("driver")
        .and_then(Value::as_str)
        .unwrap_or("local")
        .to_string();
    let source = cfg
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let capacity_bytes = cfg.get("sizeInBytes").and_then(Value::as_u64).unwrap_or(0);
    let used_bytes = if source.is_empty() {
        0
    } else {
        std::fs::metadata(&source).map(|m| m.len()).unwrap_or(0)
    };
    let ref_count = refs.get(name.as_str()).copied().unwrap_or(0);
    Volume {
        name,
        driver,
        mountpoint: source,
        size: bytes_to_gib(capacity_bytes),
        used: bytes_to_gib(used_bytes),
        unit: "GiB".into(),
        refs: ref_count,
    }
}

// Scan container ls output for volume mounts and return a name → count map.
// Apple's container reports each mount as `{"source": "<name>", "type":
// "volume", ...}` inside `configuration.mounts`. Bind mounts (`type:
// "bind"`) are skipped.
fn volume_ref_counts(ls_bytes: &[u8]) -> std::collections::HashMap<String, u32> {
    use std::collections::HashMap;
    let raw: Vec<Value> = serde_json::from_slice(ls_bytes).unwrap_or_default();
    let mut out: HashMap<String, u32> = HashMap::new();
    for c in raw {
        let mounts = c
            .get("configuration")
            .and_then(|cfg| cfg.get("mounts"))
            .and_then(Value::as_array);
        let Some(mounts) = mounts else { continue };
        for m in mounts {
            // 0.x: `"type": "volume"` with the volume name in `source`.
            // 1.0: `"type": {"volume": {"name": "...", ...}}` and `source`
            // holds the backing image path instead of the name.
            match m.get("type") {
                Some(Value::String(kind)) if kind == "volume" => {
                    if let Some(src) = m.get("source").and_then(Value::as_str) {
                        *out.entry(src.to_string()).or_insert(0) += 1;
                    }
                }
                Some(Value::Object(kind)) => {
                    if let Some(name) = kind
                        .get("volume")
                        .and_then(|vol| vol.get("name"))
                        .and_then(Value::as_str)
                    {
                        *out.entry(name.to_string()).or_insert(0) += 1;
                    }
                }
                _ => {}
            }
        }
    }
    out
}

pub async fn list_networks() -> Result<Vec<Network>> {
    let bytes = run(&["network", "ls", "--format", "json"]).await?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    Ok(raw.into_iter().map(parse_network).collect())
}

fn parse_network(v: Value) -> Network {
    // 0.x called the nested block `config`; 1.0 renamed it
    // `configuration` and added `ipv4Gateway` to `status`.
    let cfg = v
        .get("configuration")
        .or_else(|| v.get("config"))
        .cloned()
        .unwrap_or(Value::Null);
    let name = cfg
        .get("name")
        .or_else(|| v.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let mode = cfg
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    // Apple's CLI returns "running" for an up network; the UI treats
    // "active" as the up state. Normalize so the status dot lights up.
    let raw_state = v.get("state").and_then(Value::as_str).unwrap_or("active");
    let state = if matches!(raw_state, "running" | "active" | "up") {
        "active"
    } else {
        "stopped"
    }
    .to_string();
    let status = v.get("status");
    let subnet = status
        .and_then(|s| s.get("ipv4Subnet").or_else(|| s.get("ipv6Subnet")))
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_string();
    let gateway = status
        .and_then(|s| s.get("ipv4Gateway"))
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_string();
    Network {
        id: name.clone(),
        name,
        mode,
        state,
        subnet,
        gateway,
        dns: vec![],
        containers: 0,
    }
}

// ─── Inspect / actions ────────────────────────────────────────────────

pub async fn inspect(id: &str) -> Result<String> {
    inspect_args(&["inspect", id]).await
}

pub async fn inspect_volume(name: &str) -> Result<String> {
    inspect_args(&["volume", "inspect", name]).await
}

pub async fn inspect_network(id: &str) -> Result<String> {
    inspect_args(&["network", "inspect", id]).await
}

pub async fn inspect_image(reference: &str) -> Result<String> {
    inspect_args(&["image", "inspect", reference]).await
}

async fn inspect_args(args: &[&str]) -> Result<String> {
    let bytes = run(args).await?;
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(v) => Ok(serde_json::to_string_pretty(&v)
            .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned())),
        Err(_) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
    }
}

pub async fn start(id: &str) -> Result<()> {
    run(&["start", id]).await.map(|_| ())
}
pub async fn stop(id: &str) -> Result<()> {
    run(&["stop", id]).await.map(|_| ())
}
pub async fn kill(id: &str) -> Result<()> {
    run(&["kill", id]).await.map(|_| ())
}
pub async fn delete(id: &str) -> Result<()> {
    run(&["delete", id]).await.map(|_| ())
}
pub async fn restart(id: &str) -> Result<()> {
    let _ = run(&["stop", id]).await;
    run(&["start", id]).await.map(|_| ())
}

// Args used to build a `container run -d` invocation. All optional except the
// image reference. Matches the fields the UI's RunImageModal collects.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunArgs {
    pub image: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub ports: Vec<String>, // "host:container" each
    #[serde(default)]
    pub env: Vec<String>, // "KEY=value" each
    #[serde(default)]
    pub command: Option<String>, // free-form; split on spaces
}

pub async fn run_image(args: RunArgs) -> Result<String> {
    let mut argv: Vec<String> = vec!["run".into(), "-d".into()];
    if let Some(n) = args.name.as_deref().filter(|s| !s.is_empty()) {
        argv.push("--name".into());
        argv.push(n.into());
    }
    for p in &args.ports {
        if !p.is_empty() {
            argv.push("-p".into());
            argv.push(p.clone());
        }
    }
    for e in &args.env {
        if !e.is_empty() {
            argv.push("-e".into());
            argv.push(e.clone());
        }
    }
    argv.push(args.image.clone());
    if let Some(cmd) = args.command.as_deref().filter(|s| !s.is_empty()) {
        for tok in cmd.split_whitespace() {
            argv.push(tok.into());
        }
    }
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    // First-run image pulls + VM provisioning blow past the default
    // inspect-sized timeout; use the long envelope so the modal isn't
    // killed out from under the user mid-pull.
    let bytes = run_with_timeout(&argv_ref, RUN_LONG_TIMEOUT).await?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

pub async fn delete_image(reference: &str) -> Result<()> {
    run(&["image", "delete", reference]).await.map(|_| ())
}

pub async fn tag_image(source: &str, target: &str) -> Result<()> {
    run(&["image", "tag", source, target]).await.map(|_| ())
}

// ─── Prune (1.0 parity) ───────────────────────────────────────────────
// All four prune commands are non-interactive in Apple's CLI. The raw
// stdout (a list of removed ids / reclaimed space) is returned so the
// UI can toast a meaningful summary. `image prune` intentionally omits
// `--all`: dangling-only is the safe default for a one-click button.

pub async fn prune_containers() -> Result<String> {
    let bytes = run(&["prune"]).await?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}
pub async fn prune_images() -> Result<String> {
    let bytes = run(&["image", "prune"]).await?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}
pub async fn prune_volumes() -> Result<String> {
    let bytes = run(&["volume", "prune"]).await?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}
pub async fn prune_networks() -> Result<String> {
    let bytes = run(&["network", "prune"]).await?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

// ─── Disk usage (`system df`, 1.0) ────────────────────────────────────

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiskUsageRow {
    pub total: u64,
    pub active: u64,
    pub size_in_bytes: u64,
    pub reclaimable: u64,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiskUsage {
    pub images: DiskUsageRow,
    pub containers: DiskUsageRow,
    pub volumes: DiskUsageRow,
}

pub async fn system_df() -> Result<DiskUsage> {
    let bytes = run(&["system", "df", "--format", "json"]).await?;
    serde_json::from_slice(&bytes).context("parse `system df` json")
}
pub async fn delete_volume(name: &str) -> Result<()> {
    run(&["volume", "delete", name]).await.map(|_| ())
}
pub async fn delete_network(id: &str) -> Result<()> {
    run(&["network", "delete", id]).await.map(|_| ())
}

// Open a Terminal.app window running `<runtime> exec -it <id> /bin/sh`.
// macOS-only because the host project (Apple's `container` runtime) is
// macOS-only.
pub fn exec_in_terminal(id: &str) -> Result<()> {
    let runtime_bin = bin();
    let cmd_line = format!(
        "{} exec -it {} /bin/sh",
        shell_quote(&runtime_bin),
        shell_quote(id)
    );
    let script = format!(
        r#"tell application "Terminal" to do script "{}""#,
        cmd_line.replace('\\', "\\\\").replace('"', "\\\"")
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .with_context(|| "failed to spawn osascript")?;
    let _ = std::process::Command::new("osascript")
        .args(["-e", r#"tell application "Terminal" to activate"#])
        .status();
    Ok(())
}

fn shell_quote(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._/=:".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

// ─── Streaming spawners ───────────────────────────────────────────────
//
// Each returns a spawned `Child` and a stream-consumer task. The caller
// (commands.rs) merges stdout+stderr into per-line callbacks and emits Tauri
// events. Callers should hold onto the Child to abort with `start_kill()`.

pub fn spawn(args: &[&str]) -> Result<Child> {
    Command::new(bin())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("spawn `{} {}`", bin(), args.join(" ")))
}

// Drain stdout+stderr line-by-line, invoking `on_line` for each. Returns when
// the child exits; the child handle is consumed.
pub async fn drain_lines<F>(mut child: Child, mut on_line: F) -> Result<std::process::ExitStatus>
where
    F: FnMut(String) + Send + 'static,
{
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let tx_out = tx.clone();
    let t_out = tokio::spawn(async move {
        if let Some(out) = stdout {
            let mut lines = BufReader::new(out).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_out.send(line);
            }
        }
    });
    let tx_err = tx.clone();
    let t_err = tokio::spawn(async move {
        if let Some(err) = stderr {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_err.send(line);
            }
        }
    });
    drop(tx);
    let pump = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            on_line(line);
        }
    });
    let status = child.wait().await?;
    let _ = t_out.await;
    let _ = t_err.await;
    let _ = pump.await;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn patch_path_prepends_missing_dirs() {
        let bare = "/usr/bin:/bin"; // launchd default minus a couple
        let patched = patch_path(bare, &["/usr/local/bin", "/opt/homebrew/bin"]).unwrap();
        // Both prepended in declaration order, original PATH preserved at the tail.
        assert_eq!(patched, "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin");
    }

    #[test]
    fn patch_path_skips_already_present_dirs() {
        let already = "/opt/homebrew/bin:/usr/local/bin:/usr/bin";
        // Both needed dirs are already there — no change required.
        assert_eq!(
            patch_path(already, &["/usr/local/bin", "/opt/homebrew/bin"]),
            None,
        );
    }

    #[test]
    fn patch_path_partial_overlap() {
        // Only /usr/local/bin is missing; /opt/homebrew/bin already present.
        let mixed = "/opt/homebrew/bin:/usr/bin";
        let patched = patch_path(mixed, &["/usr/local/bin", "/opt/homebrew/bin"]).unwrap();
        assert_eq!(patched, "/usr/local/bin:/opt/homebrew/bin:/usr/bin");
    }

    #[test]
    fn patch_path_handles_empty_current() {
        let patched = patch_path("", &["/usr/local/bin"]).unwrap();
        assert_eq!(patched, "/usr/local/bin");
    }

    #[test]
    fn parse_size_to_gib_handles_units() {
        assert!((parse_size_to_gib("1 GB") - 1.0).abs() < 1e-6);
        assert!((parse_size_to_gib("512 MB") - 0.5).abs() < 1e-6);
        assert!((parse_size_to_gib("70.5 MB") - (70.5 / 1024.0)).abs() < 1e-6);
        assert!((parse_size_to_gib("2 TB") - 2048.0).abs() < 1e-6);
        assert_eq!(parse_size_to_gib(""), 0.0);
    }

    #[test]
    fn normalize_status_buckets_variants() {
        assert_eq!(normalize_status("running"), "running");
        assert_eq!(normalize_status("Up"), "running");
        assert_eq!(normalize_status("Paused"), "paused");
        assert_eq!(normalize_status("exited"), "exited");
        assert_eq!(normalize_status("created"), "exited");
        assert_eq!(normalize_status("dead"), "dead");
    }

    #[test]
    fn format_duration_picks_largest_unit() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(125), "2m");
        assert_eq!(format_duration(3725), "1h 02m");
        assert_eq!(format_duration(90061), "1d 01h");
    }

    #[test]
    fn apple_epoch_offset_matches_2001() {
        // 2001-01-01T00:00:00Z = unix 978307200
        assert_eq!(apple_epoch_to_unix(0.0), 978_307_200);
    }

    #[test]
    fn shell_quote_escapes_only_when_needed() {
        assert_eq!(shell_quote("simple"), "simple");
        assert_eq!(shell_quote("path/to/bin"), "path/to/bin");
        assert_eq!(shell_quote("has space"), "'has space'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    // Exercise the streaming line pump end-to-end against a controlled
    // child. Captures the same path used by start_log_stream / start_pull.
    #[tokio::test]
    async fn drain_lines_collects_all_output() {
        let child = Command::new("sh")
            .args([
                "-c",
                "printf 'alpha\\nbravo\\ncharlie\\n'; printf 'errline\\n' >&2",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn sh");
        let collected = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink = collected.clone();
        let status = drain_lines(child, move |line| sink.lock().unwrap().push(line))
            .await
            .expect("drain ok");
        assert!(status.success());
        let lines = collected.lock().unwrap().clone();
        assert!(
            lines.iter().any(|l| l == "alpha"),
            "missing stdout line: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l == "bravo"),
            "missing stdout line: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l == "charlie"),
            "missing stdout line: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l == "errline"),
            "missing stderr line: {lines:?}"
        );
    }

    #[test]
    fn parse_container_1_0_shape() {
        // Verbatim structure from `container ls --format json` on 1.0.0:
        // status is an object, startedDate is RFC3339 under status, the
        // hostname moved to networks[0].options, and mount types are
        // objects keyed by kind.
        let v = json!({
            "id": "cgui-smoke",
            "status": {
                "state": "running",
                "startedDate": "2026-06-12T13:00:26Z",
                "networks": [{ "hostname": "cgui-smoke", "ipv4Address": "192.168.64.2/24" }]
            },
            "configuration": {
                "id": "cgui-smoke",
                "creationDate": "2026-06-12T13:00:24Z",
                "image": { "reference": "docker.io/library/alpine:latest" },
                "resources": { "cpus": 4, "memoryInBytes": 1073741824u64 },
                "publishedPorts": [],
                "networks": [{ "network": "default", "options": { "hostname": "cgui-smoke", "mtu": 1280 } }],
                "initProcess": { "executable": "sleep", "arguments": ["120"] },
                "labels": {}
            }
        });
        let c = parse_container(v);
        assert_eq!(c.status, "running");
        assert_eq!(c.name, "cgui-smoke");
        assert_eq!(c.image, "docker.io/library/alpine:latest");
        // RFC3339 startedDate must produce a real uptime, not "—".
        assert_ne!(c.uptime, "—");
        assert_eq!(c.cmd, vec!["sleep", "120"]);
    }

    #[test]
    fn parse_image_1_0_shape() {
        let v = json!({
            "id": "5b10f432ef3da1b8d4c7eb6c487f2f5a8f096bc91145e68878dd4a5019afde11",
            "configuration": {
                "creationDate": "2026-04-15T20:00:31Z",
                "descriptor": { "digest": "sha256:5b10f432ef3da1b8d4c7eb6c487f2f5a8f096bc91145e68878dd4a5019afde11" },
                "name": "docker.io/library/alpine:latest"
            },
            "variants": [
                { "platform": { "architecture": "amd64", "os": "linux" }, "size": 3865822u64 },
                { "platform": { "architecture": "arm64", "os": "linux" }, "size": 3999999u64 }
            ]
        });
        let img = parse_image(v);
        assert_eq!(img.reference, "docker.io/library/alpine:latest");
        assert_eq!(img.created, "2026-04-15T20:00:31Z");
        assert_eq!(img.id, "sha256:5b10f432");
        // Variant sizes are summed bytes → GiB.
        assert!((img.size - ((3865822.0 + 3999999.0) / (1024.0 * 1024.0 * 1024.0))).abs() < 1e-9);
    }

    #[test]
    fn parse_image_0_x_shape_still_works() {
        let v = json!({
            "reference": "docker.io/library/redis:7",
            "fullSize": "142 MB",
            "createdAt": "2026-01-01T00:00:00Z",
            "descriptor": { "digest": "sha256:abcdef0123456789" }
        });
        let img = parse_image(v);
        assert_eq!(img.reference, "docker.io/library/redis:7");
        assert_eq!(img.created, "2026-01-01T00:00:00Z");
        assert!((img.size - (142.0 / 1024.0)).abs() < 1e-6);
    }

    #[test]
    fn parse_volume_1_0_shape() {
        let v = json!({
            "id": "cgui-test-vol",
            "configuration": {
                "creationDate": "2026-06-12T13:00:15Z",
                "driver": "local",
                "format": "ext4",
                "name": "cgui-test-vol",
                "sizeInBytes": 549755813888u64,
                "source": "/nonexistent/volumes/cgui-test-vol/volume.img"
            }
        });
        let refs = std::collections::HashMap::from([("cgui-test-vol".to_string(), 2u32)]);
        let vol = parse_volume(v, &refs);
        assert_eq!(vol.name, "cgui-test-vol");
        assert_eq!(vol.driver, "local");
        assert_eq!(vol.refs, 2);
        assert!((vol.size - 512.0).abs() < 1e-6);
    }

    #[test]
    fn parse_network_1_0_shape() {
        let v = json!({
            "id": "default",
            "configuration": {
                "creationDate": "2026-04-11T19:00:16Z",
                "mode": "nat",
                "name": "default"
            },
            "status": {
                "ipv4Gateway": "192.168.64.1",
                "ipv4Subnet": "192.168.64.0/24",
                "ipv6Subnet": "fdea:57c1:ba3e:6cbe::/64"
            }
        });
        let n = parse_network(v);
        assert_eq!(n.name, "default");
        assert_eq!(n.mode, "nat");
        assert_eq!(n.state, "active");
        assert_eq!(n.subnet, "192.168.64.0/24");
        assert_eq!(n.gateway, "192.168.64.1");
    }

    #[test]
    fn volume_ref_counts_handles_both_mount_shapes() {
        let ls = json!([
            { "configuration": { "mounts": [
                // 1.0: type object with the volume name nested inside.
                { "destination": "/data", "source": "/path/volume.img",
                  "type": { "volume": { "name": "vol-a", "format": "ext4" } } },
                // 1.0: non-volume mount types are skipped.
                { "destination": "/run", "source": "", "type": { "tmpfs": {} } }
            ] } },
            { "configuration": { "mounts": [
                // 0.x: type string with the volume name in source.
                { "destination": "/data", "source": "vol-a", "type": "volume" },
                { "destination": "/host", "source": "/host", "type": "bind" }
            ] } }
        ]);
        let counts = volume_ref_counts(serde_json::to_vec(&ls).unwrap().as_slice());
        assert_eq!(counts.get("vol-a"), Some(&2));
        assert_eq!(counts.len(), 1);
    }

    #[test]
    fn parse_container_apple_shape() {
        // Hand-crafted from real `container ls --all --format json` output.
        let v = json!({
            "status": "running",
            "startedDate": 0.0, // Apple epoch — translates to unix 978307200
            "configuration": {
                "id": "abcdef1234567890",
                "image": { "reference": "docker.io/library/alpine:latest" },
                "resources": { "memoryInBytes": 1073741824u64 },
                "publishedPorts": [{ "hostPort": 8080, "containerPort": 80, "proto": "tcp" }],
                "networks": [{ "hostname": "myapp" }],
                "initProcess": { "executable": "sh", "arguments": ["-c", "true"] },
                "labels": { "com.docker.compose.project": "demo" }
            }
        });
        let c = parse_container(v);
        assert_eq!(c.id, "abcdef12");
        assert_eq!(c.name, "myapp");
        assert_eq!(c.image, "docker.io/library/alpine:latest");
        assert_eq!(c.status, "running");
        assert_eq!(c.ports, vec!["8080:80"]);
        assert_eq!(c.cmd, vec!["sh", "-c", "true"]);
        assert_eq!(c.stack.as_deref(), Some("demo"));
        // Memory limit reported in GiB.
        assert!((c.mem.limit - 1.0).abs() < 1e-6);
    }
}
