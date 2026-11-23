// Thin async wrapper around the active container runtime CLI (defaults to
// Apple's `container`). Mirrors the parsing logic in cgui's container.rs so
// the rest of the app gets typed `Container` values regardless of which CLI
// is active.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::model::{Container, Image, MemUsage, Network, Volume};

/// Validate a user-supplied container/machine name against the rules
/// Apple's `container` 1.4+ enforces (`ManagedContainer.nameValid`):
/// `^[a-zA-Z0-9][a-zA-Z0-9_.-]+$`, i.e. at least 2 characters, starting
/// with a letter or digit, and 63 characters or fewer. Older CLIs accept
/// this and more, so validating up front is purely additive.
///
/// Without this check the CLI rejects an invalid name with an unhelpful
/// "container ID `…` is not a valid container ID" — this gives a
/// specific, actionable reason instead.
pub fn validate_entity_name(kind: &str, name: &str) -> Result<()> {
    const MAX_LEN: usize = 63;
    let len = name.chars().count();
    if len > MAX_LEN {
        return Err(anyhow!(
            "{kind} name `{name}` is {len} characters; Apple container 1.4+ allows at most {MAX_LEN}"
        ));
    }
    if len < 2 {
        return Err(anyhow!(
            "{kind} name `{name}` is too short; Apple container 1.4+ requires at least 2 characters"
        ));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphanumeric() {
        return Err(anyhow!(
            "{kind} name `{name}` must start with a letter or digit and contain only letters, digits, `_`, `.`, `-`"
        ));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')) {
        return Err(anyhow!(
            "{kind} name `{name}` must start with a letter or digit and contain only letters, digits, `_`, `.`, `-`"
        ));
    }
    Ok(())
}

/// Defence in depth for values handed to the CLI as *positional*
/// arguments (ids, names, image references, paths). We never go through a
/// shell, but a value such as `--all` supplied as an id would still be read
/// by the CLI as a flag. Reject empty values and anything starting with `-`.
/// Users with a file literally named `-foo` can pass `./-foo`.
///
/// Called at the `runtime.rs` function boundary (and from the few
/// `commands.rs` / `pty.rs` spawners) so every caller is covered.
pub fn reject_flag_like(kind: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(anyhow!("{kind} must not be empty"));
    }
    if value.starts_with('-') {
        let hint = if kind.ends_with("path") || kind == "build context" {
            " (prefix it with `./` to use a path that starts with `-`)"
        } else {
            ""
        };
        return Err(anyhow!(
            "{kind} `{value}` must not start with `-` (it would be read as a command-line flag){hint}"
        ));
    }
    Ok(())
}

/// Prepend the standard macOS install locations to our process's PATH
/// so spawned children can resolve `container` /
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
    #[serde(rename = "cpuUsageUsec")]
    pub cpu_usage_usec: u64,
    #[serde(rename = "memoryUsageBytes")]
    pub memory_usage_bytes: u64,
    #[serde(rename = "memoryLimitBytes")]
    pub memory_limit_bytes: u64,
    #[serde(rename = "networkRxBytes", default)]
    pub network_rx_bytes: u64,
    #[serde(rename = "networkTxBytes", default)]
    pub network_tx_bytes: u64,
    #[serde(rename = "blockReadBytes", default)]
    pub block_read_bytes: u64,
    #[serde(rename = "blockWriteBytes", default)]
    pub block_write_bytes: u64,
    /// CPU use as a fraction of ONE core (1.0 = one core fully busy),
    /// sampled by the CLI over ~2 s. Present since container 1.7.1; older
    /// CLIs omit it and state.rs falls back to `cpu_usage_usec` deltas.
    #[serde(rename = "cpuUtilization", default)]
    pub cpu_utilization: Option<f64>,
}

const RUN_TIMEOUT: Duration = Duration::from_secs(8);

/// Timeout for `container run` invocations. Even with `-d` (detach),
/// Apple's `container` may need to pull a missing image and stand up
/// the lightweight VM, which is comfortably out of the 8 s envelope
/// the rest of `run` uses for fast inspect-style queries. 3 minutes
/// is generous enough for a first-pull of a small base image like
/// alpine on a slow connection.
const RUN_LONG_TIMEOUT: Duration = Duration::from_secs(180);

/// Timeout for container lifecycle verbs (start/stop/kill/delete/restart).
/// A graceful stop or a cold VM start routinely exceeds the 8 s fast-query
/// envelope, which would report "timed out" while the work is still running.
const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Long timeout for `container run`, exposed for stacks.
pub(crate) fn run_long_timeout() -> Duration {
    RUN_LONG_TIMEOUT
}

/// Lifecycle timeout, exposed for stacks.
pub(crate) fn lifecycle_timeout() -> Duration {
    LIFECYCLE_TIMEOUT
}

fn bin() -> &'static str {
    "container"
}

/// Public accessor for the currently-active runtime binary name. Used
/// by callers (e.g. pty.rs) that want to compose their own argv without
/// going through the `run`/`spawn` helpers.
pub fn current_bin() -> &'static str {
    bin()
}

// True when the binary is reachable on PATH. Used so the UI can fall back to
// fixtures on machines without Apple's container runtime installed.
pub async fn available() -> bool {
    Command::new(bin())
        .kill_on_drop(true)
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Render an argv for logs and error text with environment values hidden:
/// `-e KEY=value` / `--env KEY=value` / `--env=KEY=value` become `KEY=***`.
/// Display only; the argv actually executed is never modified.
pub fn mask_env_args<S: AsRef<str>>(args: &[S]) -> Vec<String> {
    fn mask(kv: &str) -> String {
        match kv.split_once('=') {
            Some((k, _)) => format!("{k}=***"),
            None => kv.to_string(), // `-e KEY` (inherit) carries no value
        }
    }
    let mut out = Vec::with_capacity(args.len());
    let mut mask_next = false;
    for a in args {
        let a = a.as_ref();
        if mask_next {
            out.push(mask(a));
            mask_next = false;
        } else if a == "-e" || a == "--env" {
            out.push(a.to_string());
            mask_next = true;
        } else if let Some(kv) = a.strip_prefix("--env=") {
            out.push(format!("--env={}", mask(kv)));
        } else {
            out.push(a.to_string());
        }
    }
    out
}

async fn run(args: &[&str]) -> Result<Vec<u8>> {
    run_with_timeout(args, RUN_TIMEOUT).await
}

/// Variant of `run` with a caller-chosen timeout for operations that
/// are inherently slower than fast inspect / list queries (image pull,
/// `container run`, etc.). Same error envelope as `run`.
pub(crate) async fn run_with_timeout(args: &[&str], timeout: Duration) -> Result<Vec<u8>> {
    let out = run_output(args, timeout).await?;
    if !out.status.success() {
        return Err(anyhow!(
            "`{} {}` exited {}: {}",
            bin(),
            mask_env_args(args).join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

/// Spawn the runtime and collect its output regardless of exit status.
/// Errors only on spawn failure or timeout; callers decide what a
/// non-zero exit means.
async fn run_output(args: &[&str], timeout: Duration) -> Result<std::process::Output> {
    let fut = Command::new(bin()).args(args).kill_on_drop(true).output();
    tokio::time::timeout(timeout, fut)
        .await
        .map_err(|_| {
            anyhow!(
                "`{} {}` timed out after {}s",
                bin(),
                mask_env_args(args).join(" "),
                timeout.as_secs()
            )
        })?
        .with_context(|| {
            format!(
                "failed to spawn `{} {}`",
                bin(),
                mask_env_args(args).join(" ")
            )
        })
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

/// Match a stats row to a container by exact id. `Container.id` is the full
/// CLI id and `container stats` reports the same id; prefix matching would
/// conflate containers that share a prefix (e.g. `stack_web` / `stack_worker`).
pub fn match_stat<'a>(stats: &'a [StatRow], id: &str) -> Option<&'a StatRow> {
    stats.iter().find(|s| s.id == id)
}

/// Display-only abbreviation of a container id (first 8 chars).
pub fn short_container_id(id: &str) -> String {
    id.chars().take(8).collect()
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
    // `status` is `{"state": "running", "startedDate": "...", "networks": [...]}`.
    let status = v
        .get("status")
        .and_then(|s| s.get("state"))
        .and_then(Value::as_str)
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

    // Display-only short id; `Container.id` stays the full CLI id.
    let short_id = short_container_id(&id);

    // `status.startedDate` (RFC3339) is present while running; the creation
    // time is `configuration.creationDate`.
    let started_at = v
        .get("status")
        .and_then(|s| s.get("startedDate"))
        .and_then(Value::as_str)
        .and_then(crate::timeutil::parse_rfc3339_unix);
    let started_unix = started_at;
    let started_iso = v
        .get("status")
        .and_then(|s| s.get("startedDate"))
        .or_else(|| cfg.get("creationDate"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let normalized = normalize_status(&status);
    let uptime = if normalized == "running" {
        match started_unix {
            Some(t) => format_uptime_from_unix(t),
            None => "—".into(),
        }
    } else {
        "—".into()
    };

    // Hostname lives at `networks[0].options.hostname`.
    let name = cfg
        .get("hostname")
        .and_then(Value::as_str)
        .or_else(|| {
            cfg.get("networks")
                .and_then(|n| n.get(0))
                .and_then(|n| n.get("options"))
                .and_then(|o| o.get("hostname"))
                .and_then(Value::as_str)
        })
        .unwrap_or(&id)
        .to_string();

    let cmd = cfg
        .get("initProcess")
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
        id,
        short_id,
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

fn format_uptime_from_unix(unix_secs: i64) -> String {
    let secs = (crate::timeutil::now_unix() - unix_secs).max(0);
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
// variants so the UI's status-pill switch hits.
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

/// Result of `image ls`: the readable images plus an optional non-blocking
/// warning when the CLI skipped unreadable ones.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImageListing {
    pub images: Vec<Image>,
    pub warning: Option<String>,
}

pub async fn list_images() -> Result<Vec<Image>> {
    list_images_checked().await.map(|l| l.images)
}

/// `container image ls`. Since container 1.6 the CLI skips unreadable
/// images with a warning, prints the readable ones as valid JSON, then
/// exits 1 ("failed to read N image(s)"). Treat that as a partial
/// success so one corrupt image doesn't blank the whole view.
pub async fn list_images_checked() -> Result<ImageListing> {
    let args = ["image", "ls", "--format", "json"];
    let out = run_output(&args, RUN_TIMEOUT).await?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    parse_image_ls(out.status.success(), &out.stdout, &stderr, &out.status)
        .map_err(|e| e.context(format!("`{} {}`", bin(), args.join(" "))))
}

/// Pure core of `list_images_checked`. A non-zero exit is accepted only
/// when stdout is a JSON array (the 1.6+ partial-output behaviour);
/// anything else (services down, garbage) stays an error.
fn parse_image_ls(
    success: bool,
    stdout: &[u8],
    stderr: &str,
    status: &dyn std::fmt::Display,
) -> Result<ImageListing> {
    let parsed = serde_json::from_slice::<Vec<Value>>(stdout);
    match (success, parsed) {
        (true, parsed) => {
            let raw = parsed.context("parse `image ls` json")?;
            Ok(ImageListing {
                images: raw.into_iter().map(parse_image).collect(),
                warning: None,
            })
        }
        (false, Ok(raw)) => {
            let detail = stderr.trim();
            let lead = match failed_read_count(detail) {
                Some(1) => "1 image couldn't be read".to_string(),
                Some(n) => format!("{n} images couldn't be read"),
                None => "Some images couldn't be read".to_string(),
            };
            let warning = if detail.is_empty() {
                lead
            } else {
                format!("{lead}: {detail}")
            };
            Ok(ImageListing {
                images: raw.into_iter().map(parse_image).collect(),
                warning: Some(warning),
            })
        }
        (false, Err(_)) => Err(anyhow!("exited {status}: {}", stderr.trim())),
    }
}

/// N from the CLI's "failed to read N image(s)" summary line.
fn failed_read_count(stderr: &str) -> Option<usize> {
    let marker = "failed to read ";
    let rest = &stderr[stderr.find(marker)? + marker.len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn parse_image(v: Value) -> Image {
    // Metadata is nested under `configuration` (`name`, `creationDate`,
    // `descriptor`) with per-platform byte sizes in `variants[].size`.
    let cfg = v.get("configuration");
    let reference = cfg
        .and_then(|c| c.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let bytes: u64 = v
        .get("variants")
        .and_then(Value::as_array)
        .map(|vs| {
            vs.iter()
                .filter_map(|p| p.get("size").and_then(Value::as_u64))
                .sum()
        })
        .unwrap_or(0);
    let size = bytes_to_gib(bytes);
    let digest = cfg
        .and_then(|c| c.get("descriptor"))
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
    // `name`/`driver`/`source`/`sizeInBytes` live under `configuration`.
    let cfg = v.get("configuration").unwrap_or(&Value::Null);
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
// Each volume mount is `{"type": {"volume": {"name": "<name>", ...}}, ...}`
// inside `configuration.mounts`; other mount kinds are skipped.
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
            // Mounts are `"type": {"volume": {"name": "...", ...}}`; other
            // kinds (`tmpfs`, `virtiofs`, ...) are skipped. `source` holds
            // the backing image path, not the volume name.
            if let Some(name) = m
                .get("type")
                .and_then(|t| t.get("volume"))
                .and_then(|vol| vol.get("name"))
                .and_then(Value::as_str)
            {
                *out.entry(name.to_string()).or_insert(0) += 1;
            }
        }
    }
    out
}

pub async fn list_networks() -> Result<Vec<Network>> {
    // Mirrors `list_volumes` above: fetch the network list and the
    // container list in parallel, then derive per-network attached-
    // container counts from the latter.
    let (net_res, ctr_res) = tokio::join!(
        run(&["network", "ls", "--format", "json"]),
        run(&["ls", "--all", "--format", "json"]),
    );
    let bytes = net_res?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    // A failed `ls` just means we can't compute counts — fall back to 0
    // rather than failing the whole network list over it.
    let refs = ctr_res.map(|b| network_ref_counts(&b)).unwrap_or_default();
    Ok(raw.into_iter().map(|n| parse_network(n, &refs)).collect())
}

// Scan container ls output for attached networks and return a
// name → running-container count map.
//
// `status.networks[]` is the source of truth: each entry is `{"network":
// "<name>", "hostname": ..., "ipv4Address": ..., ...}` for a container that
// is actually attached right now (`[]` when stopped). `configuration.networks[]`
// lists the networks a container *would* attach to on start, so counting it
// would count stopped containers as attached.
fn network_ref_counts(ls_bytes: &[u8]) -> std::collections::HashMap<String, u32> {
    use std::collections::HashMap;
    let raw: Vec<Value> = serde_json::from_slice(ls_bytes).unwrap_or_default();
    let mut out: HashMap<String, u32> = HashMap::new();
    for c in raw {
        let Some(nets) = c
            .get("status")
            .and_then(|s| s.get("networks"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for n in nets {
            if let Some(name) = n.get("network").and_then(Value::as_str) {
                *out.entry(name.to_string()).or_insert(0) += 1;
            }
        }
    }
    out
}

fn parse_network(v: Value, refs: &std::collections::HashMap<String, u32>) -> Network {
    let cfg = v.get("configuration").cloned().unwrap_or(Value::Null);
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
    let container_count = refs.get(name.as_str()).copied().unwrap_or(0);
    Network {
        id: name.clone(),
        name,
        mode,
        state,
        subnet,
        gateway,
        dns: vec![],
        containers: container_count,
    }
}

// ─── Inspect / actions ────────────────────────────────────────────────

pub async fn inspect(id: &str) -> Result<String> {
    reject_flag_like("container id", id)?;
    inspect_args(&["inspect", id]).await
}

pub async fn inspect_volume(name: &str) -> Result<String> {
    reject_flag_like("volume name", name)?;
    inspect_args(&["volume", "inspect", name]).await
}

pub async fn inspect_network(id: &str) -> Result<String> {
    reject_flag_like("network name", id)?;
    inspect_args(&["network", "inspect", id]).await
}

pub async fn inspect_image(reference: &str) -> Result<String> {
    reject_flag_like("image reference", reference)?;
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
    reject_flag_like("container id", id)?;
    run_with_timeout(&["start", id], LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}
pub async fn stop(id: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    run_with_timeout(&["stop", id], LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}
/// Kill with an optional signal (`container kill -s <signal> <id>`).
/// `None` keeps the CLI's default (SIGKILL).
pub async fn kill(id: &str, signal: Option<&str>) -> Result<()> {
    reject_flag_like("container id", id)?;
    if let Some(sig) = signal.filter(|s| !s.is_empty()) {
        reject_flag_like("signal", sig)?;
    }
    match signal.filter(|s| !s.is_empty()) {
        Some(sig) => run_with_timeout(&["kill", "-s", sig, id], LIFECYCLE_TIMEOUT)
            .await
            .map(|_| ()),
        None => run_with_timeout(&["kill", id], LIFECYCLE_TIMEOUT)
            .await
            .map(|_| ()),
    }
}
pub async fn delete(id: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    run_with_timeout(&["delete", id], LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}
pub async fn restart(id: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    if let Err(e) = run_with_timeout(&["stop", id], LIFECYCLE_TIMEOUT).await {
        // Only "already stopped" is expected here; anything else (services
        // down, timeout, unknown id) must reach the user instead of being
        // masked by a confusing follow-on `start` error.
        if !is_not_running_error(&format!("{e:#}")) {
            return Err(e.context(format!("could not stop `{id}` before restarting")));
        }
    }
    run_with_timeout(&["start", id], LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}

// ─── CLI error classification ─────────────────────────────────────────
//
// Texts observed on Apple `container` 1.4.1:
//   stop/delete unknown id: `Error: internalError: "failed to stop container"
//       (cause: "notFound: "container with ID X not found"")`
//   start unknown id:       `Error: get failed: container X not found`
//   delete while running:   `... invalidState: "container X is running and
//       can not be deleted"`
//   stop on an already-stopped container succeeds (exit 0).
//   services down (every subcommand, incl. `system status`):
//       `Error: Plugins are unavailable. Start the container system services
//       and retry: container system start`
// Matching is on lowercase substrings of the whole message; ids/names can't
// contain spaces so they can't produce a false "not found" / "not running".

/// The container doesn't exist (any of stop/delete/start/inspect).
pub fn is_not_found_error(msg: &str) -> bool {
    msg.to_ascii_lowercase().contains("not found")
}

/// Stop was a no-op because the container wasn't running. 1.4.1 exits 0 in
/// this case; older CLIs reported "container is not running".
pub fn is_not_running_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("not running") || m.contains("already stopped")
}

/// A failed `stop` that leaves the container in the desired (not running)
/// state: it's gone or already stopped.
pub fn is_benign_stop_error(msg: &str) -> bool {
    is_not_found_error(msg) || is_not_running_error(msg)
}

/// A failed `delete` that leaves the container in the desired (absent)
/// state. "is running and can not be deleted" is deliberately NOT benign.
pub fn is_benign_delete_error(msg: &str) -> bool {
    is_not_found_error(msg)
}

/// The runtime's API server / plugins aren't up (`container system start`
/// needed). The binary itself works, so `available()` is still true.
pub fn is_services_down_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("plugins are unavailable") || m.contains("container system start")
}

/// Builds `container clean <id> [<id>...]` argv. Unlike start/stop/kill/
/// delete/restart above, the CLI itself accepts multiple ids in a single
/// invocation, and the UI's bulk-action bar can target several containers
/// at once, so this is a slice op rather than one-id-per-call. Pure and
/// unit-testable without touching the runtime binary.
pub fn clean_argv(ids: &[String]) -> Vec<String> {
    let mut argv = vec!["clean".to_string()];
    argv.extend(ids.iter().cloned());
    argv
}

/// `container clean <id> [<id>...]` — runs an fstrim of the container's
/// writable root filesystem and every writable block mount, returning
/// freed space in the sparse ext4 disk image back to the host. Non-
/// destructive (no files or data are removed); only valid for running
/// containers, which the CLI itself enforces ("container is not running").
///
/// Trimming can take a while on a container with a lot of accumulated
/// writable-layer garbage, so this uses the long timeout like other
/// potentially-slow lifecycle ops (image pulls, builder teardown).
pub async fn clean_containers(ids: &[String]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    for id in ids {
        reject_flag_like("container id", id)?;
    }
    let argv = clean_argv(ids);
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    run_with_timeout(&argv_ref, RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
        .map_err(map_clean_unsupported)
}

/// `container clean` is new in Apple container 1.4; older CLIs dispatch
/// subcommands as external plugins and fail with `Plugin '...' not
/// found` at exit code 64 when the plugin doesn't exist. Translate that
/// specific failure into a clear version-requirement message instead of
/// the raw plugin error; leave other failures (e.g. "container is not
/// running") untouched so the caller sees the real reason.
fn map_clean_unsupported(e: anyhow::Error) -> anyhow::Error {
    // Exit 64 alone is EX_USAGE, which argument-parse errors share; also
    // require the plugin-dispatch message so those aren't mislabelled.
    let msg = e.to_string();
    // 1.4 says `Plugin 'container-clean' not found`; 1.5+ says
    // `unknown command 'clean'`.
    if msg.contains("exit status: 64")
        && (msg.contains("Plugin 'container-clean' not found")
            || msg.contains("unknown command 'clean'"))
    {
        anyhow!("`container clean` requires Apple container 1.4 or later")
    } else {
        e
    }
}

// Args used to build a `container run -d` invocation. All optional except the
// image reference. Matches the fields the UI's RunImageModal collects;
// the advanced block (resources, mounts, identity, platform) landed
// with 1.0 parity phase 4.
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
    pub command: Option<String>, // free-form; shell-style splitting (split_command)
    #[serde(default)]
    pub cpus: Option<u32>,
    #[serde(default)]
    pub memory: Option<String>, // e.g. "2G"
    #[serde(default)]
    pub volumes: Vec<String>, // "name-or-path:containerpath" each
    #[serde(default)]
    pub labels: Vec<String>, // "key=value" each
    #[serde(default)]
    pub network: Option<String>,
    /// DNS aliases for the container on `network` (container 1.6+):
    /// emitted as `--network <net>,alias=A,alias=B`. See `network_spec`.
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub platform: Option<String>, // "os/arch[/variant]"
    #[serde(default)]
    pub rm: bool,
    #[serde(default)]
    pub rosetta: bool,
}

/// Split a command line into words the way a POSIX shell would, minus
/// every expansion (no variables, globs, substitution or operators):
///
/// * unquoted whitespace separates words;
/// * `'...'` is literal;
/// * inside `"..."` a backslash escapes only `"`, `\`, `$` and `` ` ``
///   (before any other character it is kept as-is), as in POSIX;
/// * outside quotes a backslash makes the next character literal, and
///   backslash-newline is a line continuation;
/// * `""` / `''` produce an empty word.
///
/// An unterminated quote (or a trailing backslash) is an error so the user
/// sees what is wrong instead of silently running a different command.
pub fn split_command(input: &str) -> Result<Vec<String>> {
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(ch) => cur.push(ch),
                        None => {
                            return Err(anyhow!("unterminated single quote (') in: {input}"));
                        }
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(e @ ('"' | '\\' | '$' | '`')) => cur.push(e),
                            Some('\n') => {}
                            Some(other) => {
                                cur.push('\\');
                                cur.push(other);
                            }
                            None => {
                                return Err(anyhow!("unterminated double quote (\") in: {input}"));
                            }
                        },
                        Some(ch) => cur.push(ch),
                        None => {
                            return Err(anyhow!("unterminated double quote (\") in: {input}"));
                        }
                    }
                }
            }
            '\\' => match chars.next() {
                Some('\n') => {}
                Some(e) => {
                    in_word = true;
                    cur.push(e);
                }
                None => return Err(anyhow!("dangling backslash at the end of: {input}")),
            },
            other => {
                in_word = true;
                cur.push(other);
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    Ok(words)
}

/// Validate a network DNS alias (container 1.6+, `--network net,alias=NAME`):
/// a hostname of 1-63 character DNS labels (letters, digits, inner `-`),
/// at most 253 characters in all. The CLI additionally wants a fully
/// qualified name (two or more labels, e.g. `api.example.test`) unless a
/// default DNS domain is configured, and says so itself; we don't duplicate
/// that rule because the configured domain isn't known here.
///
/// This also guarantees an alias can't smuggle `,` / `=` into the
/// `--network` option list.
pub fn validate_alias(alias: &str) -> Result<()> {
    if alias.is_empty() {
        return Err(anyhow!("network alias must not be empty"));
    }
    if alias.len() > 253 {
        return Err(anyhow!(
            "network alias `{alias}` is longer than 253 characters"
        ));
    }
    let name = alias.strip_suffix('.').unwrap_or(alias);
    for label in name.split('.') {
        let ok = (1..=63).contains(&label.len())
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !label.starts_with('-')
            && !label.ends_with('-');
        if !ok {
            return Err(anyhow!(
                "network alias `{alias}` is not a valid DNS name: each part must be 1-63 letters, digits or `-`, not starting or ending with `-`"
            ));
        }
    }
    Ok(())
}

/// The value for `--network`: the network name (or `default` when only
/// aliases were given) followed by `,alias=NAME` per alias. `None` when
/// there is neither a network nor an alias, so the CLI default applies.
/// Aliases must already be validated (`validate_alias`).
pub fn network_spec(network: Option<&str>, aliases: &[String]) -> Option<String> {
    let net = network.filter(|s| !s.is_empty());
    let aliases: Vec<&String> = aliases.iter().filter(|a| !a.is_empty()).collect();
    if aliases.is_empty() {
        return net.map(String::from);
    }
    let mut spec = net.unwrap_or("default").to_string();
    for a in aliases {
        spec.push_str(",alias=");
        spec.push_str(a);
    }
    Some(spec)
}

pub fn run_image_argv(args: &RunArgs) -> Result<Vec<String>> {
    for a in args.aliases.iter().filter(|a| !a.is_empty()) {
        validate_alias(a)?;
    }
    let mut argv: Vec<String> = vec!["run".into(), "-d".into()];
    let mut opt = |flag: &str, val: Option<&str>| {
        if let Some(v) = val.filter(|s| !s.is_empty()) {
            argv.push(flag.into());
            argv.push(v.into());
        }
    };
    opt("--name", args.name.as_deref());
    opt("--cpus", args.cpus.map(|c| c.to_string()).as_deref());
    opt("--memory", args.memory.as_deref());
    opt(
        "--network",
        network_spec(args.network.as_deref(), &args.aliases).as_deref(),
    );
    opt("--workdir", args.workdir.as_deref());
    opt("--user", args.user.as_deref());
    opt("--platform", args.platform.as_deref());
    // Re-borrow argv after the closure (NLL ends its borrow above).
    for p in args.ports.iter().filter(|s| !s.is_empty()) {
        argv.push("-p".into());
        argv.push(p.clone());
    }
    for e in args.env.iter().filter(|s| !s.is_empty()) {
        argv.push("-e".into());
        argv.push(e.clone());
    }
    for v in args.volumes.iter().filter(|s| !s.is_empty()) {
        argv.push("--volume".into());
        argv.push(v.clone());
    }
    for l in args.labels.iter().filter(|s| !s.is_empty()) {
        argv.push("--label".into());
        argv.push(l.clone());
    }
    if args.rm {
        argv.push("--rm".into());
    }
    if args.rosetta {
        argv.push("--rosetta".into());
    }
    argv.push(args.image.clone());
    if let Some(cmd) = args.command.as_deref().filter(|s| !s.is_empty()) {
        argv.extend(split_command(cmd).context("could not parse the command")?);
    }
    Ok(argv)
}

pub async fn run_image(args: RunArgs) -> Result<String> {
    reject_flag_like("image reference", &args.image)?;
    if let Some(name) = args.name.as_deref().filter(|s| !s.is_empty()) {
        validate_entity_name("container", name)?;
    }
    let argv = run_image_argv(&args)?;
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    // First-run image pulls + VM provisioning blow past the default
    // inspect-sized timeout; use the long envelope so the modal isn't
    // killed out from under the user mid-pull.
    let bytes = run_with_timeout(&argv_ref, RUN_LONG_TIMEOUT).await?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

pub async fn delete_image(reference: &str) -> Result<()> {
    reject_flag_like("image reference", reference)?;
    run(&["image", "delete", reference]).await.map(|_| ())
}

pub async fn tag_image(source: &str, target: &str) -> Result<()> {
    reject_flag_like("source image reference", source)?;
    reject_flag_like("target image reference", target)?;
    run(&["image", "tag", source, target]).await.map(|_| ())
}

/// Basic sanity check for a *new* image reference (a name the user types, as
/// opposed to one already in the image list): `[host[:port]/]repo[:tag]
/// [@digest]` with a lowercase repository, a plain tag, no whitespace, and
/// nothing flag-like. The CLI is stricter still ("invalid format for image
/// reference"); this just turns the common mistakes into a specific message.
pub fn validate_image_ref(kind: &str, reference: &str) -> Result<()> {
    reject_flag_like(kind, reference)?;
    if reference.len() > 255 {
        return Err(anyhow!(
            "{kind} `{reference}` is longer than 255 characters"
        ));
    }
    if reference
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return Err(anyhow!(
            "{kind} `{reference}` must not contain spaces or control characters"
        ));
    }
    let no_digest = reference.split('@').next().unwrap_or(reference);
    let (name, tag) = match no_digest.rfind(':') {
        // A `:` before the last `/` belongs to a registry host:port.
        Some(i) if !no_digest[i..].contains('/') => (&no_digest[..i], Some(&no_digest[i + 1..])),
        _ => (no_digest, None),
    };
    if name.is_empty() || name.ends_with('/') || name.starts_with('/') {
        return Err(anyhow!("{kind} `{reference}` has no repository name"));
    }
    if name.chars().any(|c| {
        !(c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-' | '/' | ':'))
    }) {
        return Err(anyhow!(
            "{kind} `{reference}`: the repository name must be lowercase letters, digits, `.`, `_`, `-` and `/`"
        ));
    }
    if let Some(tag) = tag {
        if tag.is_empty()
            || tag.len() > 128
            || !tag
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return Err(anyhow!(
                "{kind} `{reference}` has an invalid tag (letters, digits, `.`, `_`, `-`; up to 128 characters)"
            ));
        }
    }
    Ok(())
}

/// `container commit <id> <reference>` (1.6+): snapshot a container's
/// filesystem (running or stopped) into a new image. Copying the disk can
/// take a while on large containers, so this uses the long timeout.
/// Returns the CLI's stdout (the resolved reference).
pub async fn commit_container(id: &str, reference: &str) -> Result<String> {
    reject_flag_like("container id", id)?;
    validate_image_ref("image reference", reference)?;
    let bytes = run_with_timeout(&["commit", id, reference], RUN_LONG_TIMEOUT)
        .await
        .map_err(map_commit_unsupported)?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

/// `container commit` is new in 1.6; older CLIs fail with an
/// `unknown command`/plugin-not-found error. Say what is needed instead.
fn map_commit_unsupported(e: anyhow::Error) -> anyhow::Error {
    let msg = e.to_string();
    if msg.contains("exit status: 64")
        && (msg.contains("Plugin 'container-commit' not found")
            || msg.contains("unknown command 'commit'"))
    {
        anyhow!("`container commit` requires Apple container 1.6 or later")
    } else {
        e
    }
}

// ─── Builder VM lifecycle (`container builder …`, 1.0 parity) ─────────
// `builder status --format json` returns the builder container in the
// same shape as `ls`, so parse_container is reused. None = no builder
// instance exists yet.

pub async fn builder_status() -> Result<Option<Container>> {
    let bytes = run(&["builder", "status", "--format", "json"]).await?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).context("parse `builder status` json")?;
    Ok(raw.into_iter().next().map(parse_container))
}

/// Start (creating if needed) the builder VM. First start pulls the
/// builder shim image, so this runs on the long timeout.
pub async fn builder_start(cpus: Option<u32>, memory: Option<String>) -> Result<()> {
    let mut argv: Vec<String> = vec!["builder".into(), "start".into()];
    if let Some(c) = cpus.filter(|c| *c > 0) {
        argv.push("--cpus".into());
        argv.push(c.to_string());
    }
    if let Some(m) = memory.filter(|m| !m.is_empty()) {
        argv.push("--memory".into());
        argv.push(m);
    }
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    run_with_timeout(&argv_ref, RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn builder_stop() -> Result<()> {
    run_with_timeout(&["builder", "stop"], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn builder_delete() -> Result<()> {
    run_with_timeout(&["builder", "delete", "--force"], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

// ─── Runtime services (`container system start/stop`) ─────────────────
// Sudo-free service controls. Fixed argv (no user input), run on the
// long timeout because start provisions launchd agents + the helper VM.
// `start` is idempotent; `stop` tears the services down.

pub async fn system_start() -> Result<()> {
    clear_cli_version_cache();
    run_with_timeout(&["system", "start"], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn system_stop() -> Result<()> {
    clear_cli_version_cache();
    run_with_timeout(&["system", "stop"], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

/// Restart the runtime services (`system stop` then `system start`).
/// Fixes a CLI/apiserver version mismatch after an upgrade, where the old
/// server keeps running until it is stopped. A failed stop is tolerated
/// (nothing to stop); the start result is what matters.
pub async fn system_restart() -> Result<()> {
    let _ = system_stop().await;
    system_start().await
}

/// `client.version` / `server.version` from `container system status
/// --format json` (present since 1.4.1). Either is None when absent.
pub fn parse_system_versions(body: &str) -> (Option<String>, Option<String>) {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return (None, None);
    };
    let field = |side: &str| {
        v.get(side)
            .and_then(|o| o.get("version"))
            .and_then(Value::as_str)
            .map(String::from)
    };
    (field("client"), field("server"))
}

/// True when `container system status --format json` reports the services
/// up: a top-level `{"status": "running", ...}` (down states are
/// `"unregistered"` or `"not running"`). Anything else, including
/// unparseable output, is "not running".
pub fn parse_system_running(body: &str) -> bool {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.get("status")
                .and_then(Value::as_str)
                .map(|s| s == "running")
        })
        .unwrap_or(false)
}

/// Returns whether the runtime services are up. Never errors — a
/// non-zero exit (which `system status` returns when the services are
/// down) is reported as `false` rather than propagated, so callers can
/// render a clean stopped state.
pub async fn system_running() -> Result<bool> {
    match run(&["system", "status", "--format", "json"]).await {
        Ok(bytes) => Ok(parse_system_running(&String::from_utf8_lossy(&bytes))),
        Err(_) => Ok(false),
    }
}

// ─── CLI capability gate ──────────────────────────────────────────────
// Features that need a newer CLI (commit 1.6+, DNS aliases 1.6+, native CPU
// 1.7.1+, ...) are gated in the UI on the detected CLI version. The version
// is cached for the process lifetime; start/stop/restart of the services
// clears it so an upgrade followed by a services restart is picked up.

static CLI_VERSION: RwLock<Option<String>> = RwLock::new(None);

fn clear_cli_version_cache() {
    *CLI_VERSION.write().unwrap() = None;
}

/// Version of the CLI binary (`container --version`), e.g. `1.7.1`. `None`
/// when the binary can't be run or prints nothing parseable. Only successful
/// detections are cached.
pub async fn cli_version() -> Option<String> {
    if let Some(v) = CLI_VERSION.read().unwrap().clone() {
        return Some(v);
    }
    let bytes = run(&["--version"]).await.ok()?;
    let v = crate::updates::extract_version(&String::from_utf8_lossy(&bytes))?;
    *CLI_VERSION.write().unwrap() = Some(v.clone());
    Some(v)
}

// Args for a streamed `container build`. Mirrors the BuildModal form.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildArgs {
    pub context: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub dockerfile: Option<String>,
    #[serde(default)]
    pub build_args: Vec<String>, // KEY=value each
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub no_cache: bool,
}

pub fn build_argv(args: &BuildArgs) -> Vec<String> {
    let mut argv: Vec<String> = vec!["build".into(), "--progress".into(), "plain".into()];
    if let Some(t) = args.tag.as_deref().filter(|s| !s.is_empty()) {
        argv.push("-t".into());
        argv.push(t.into());
    }
    if let Some(f) = args.dockerfile.as_deref().filter(|s| !s.is_empty()) {
        argv.push("-f".into());
        argv.push(f.into());
    }
    for ba in args.build_args.iter().filter(|s| !s.is_empty()) {
        argv.push("--build-arg".into());
        argv.push(ba.clone());
    }
    if let Some(stage) = args.target.as_deref().filter(|s| !s.is_empty()) {
        argv.push("--target".into());
        argv.push(stage.into());
    }
    if args.no_cache {
        argv.push("--no-cache".into());
    }
    argv.push(args.context.clone());
    argv
}

// ─── Image lifecycle: push / save / load (1.0 parity) ─────────────────
// All three can move gigabytes, so they run on the long timeout. Push
// streams via spawn + drain_lines in commands.rs instead (progress UI).

pub async fn save_image(reference: &str, output: &str) -> Result<()> {
    reject_flag_like("image reference", reference)?;
    reject_flag_like("output path", output)?;
    run_with_timeout(
        &["image", "save", "-o", output, reference],
        RUN_LONG_TIMEOUT,
    )
    .await
    .map(|_| ())
}

pub async fn load_image(input: &str) -> Result<String> {
    reject_flag_like("input path", input)?;
    let bytes = run_with_timeout(&["image", "load", "-i", input], RUN_LONG_TIMEOUT).await?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

// ─── Container filesystem: copy / export (1.0 parity) ─────────────────

/// `container copy <src> <dst>` — either side may be `<id>:<path>`.
pub async fn copy_path(src: &str, dst: &str) -> Result<()> {
    reject_flag_like("source path", src)?;
    reject_flag_like("destination path", dst)?;
    run_with_timeout(&["copy", src, dst], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn export_container(id: &str, output: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    reject_flag_like("output path", output)?;
    run_with_timeout(&["export", "-o", output, id], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

// ─── Registry logins (1.0 parity) ─────────────────────────────────────

// ─── Container machines (`container machine …`, new in 1.0) ──────────
// Machines are named VMs that containers run inside. `machine list
// --format json` returns a flat shape: {id, status, cpus, memory,
// diskSize, default, createdDate}.

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Machine {
    pub id: String,
    pub status: String,
    pub cpus: u32,
    pub memory: u64,
    pub disk_size: u64,
    #[serde(rename = "default")]
    pub is_default: bool,
    pub created_date: String,
}

pub async fn machine_list() -> Result<Vec<Machine>> {
    let bytes = run(&["machine", "list", "--format", "json"]).await?;
    serde_json::from_slice(&bytes).context("parse `machine list` json")
}

/// Optional account settings for the machine's user (container 1.6+):
/// `-u/--user`, `--uid`, `--gid`, `--home`. Unset fields keep the CLI
/// defaults (the host user's name/ids, home `/home/<user>`).
#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineUserOpts {
    /// `name` or `uid[:gid]` (the CLI's `-u` format).
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub uid: Option<u32>,
    #[serde(default)]
    pub gid: Option<u32>,
    /// Absolute home directory inside the machine.
    #[serde(default)]
    pub home: Option<String>,
}

/// Validate and render `MachineUserOpts` as CLI flags (empty when unset).
pub fn machine_user_args(opts: &MachineUserOpts) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let text = |o: &Option<String>| {
        o.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    if let Some(user) = text(&opts.user) {
        reject_flag_like("machine user", &user)?;
        if user.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(anyhow!("machine user `{user}` must not contain spaces"));
        }
        out.push("--user".into());
        out.push(user);
    }
    if let Some(uid) = opts.uid {
        out.push("--uid".into());
        out.push(uid.to_string());
    }
    if let Some(gid) = opts.gid {
        out.push("--gid".into());
        out.push(gid.to_string());
    }
    if let Some(home) = text(&opts.home) {
        if !home.starts_with('/') || home.chars().any(|c| c.is_control()) {
            return Err(anyhow!(
                "machine home `{home}` must be an absolute path such as /home/dev"
            ));
        }
        out.push("--home".into());
        out.push(home);
    }
    Ok(out)
}

/// Create a machine. Pulls the image on first use, so long timeout.
/// `no_boot` creates without booting (cheaper; boots on first use).
/// `user` carries the optional 1.6+ account flags.
pub async fn machine_create(
    name: &str,
    image: &str,
    cpus: Option<u32>,
    memory: Option<String>,
    no_boot: bool,
    user: &MachineUserOpts,
) -> Result<()> {
    validate_entity_name("machine", name)?;
    reject_flag_like("image reference", image)?;
    let mut argv: Vec<String> = vec![
        "machine".into(),
        "create".into(),
        "--name".into(),
        name.into(),
    ];
    if let Some(c) = cpus.filter(|c| *c > 0) {
        argv.push("--cpus".into());
        argv.push(c.to_string());
    }
    if let Some(m) = memory.filter(|m| !m.is_empty()) {
        argv.push("--memory".into());
        argv.push(m);
    }
    if no_boot {
        argv.push("--no-boot".into());
    }
    argv.extend(machine_user_args(user)?);
    argv.push(image.into());
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    run_with_timeout(&argv_ref, RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

/// `container machine logs [--boot] [-n N] <name>`: a one-shot fetch (no
/// `--follow`). `boot` shows the VM boot log instead of stdio. `tail` limits
/// to the last N lines (clamped to 1..=5000); the CLI prints everything when
/// omitted, which can be huge, so callers pass one. A machine that has
/// never booted has no log file and the CLI says so.
pub async fn machine_logs(name: &str, boot: bool, tail: Option<u32>) -> Result<String> {
    reject_flag_like("machine name", name)?;
    let mut argv: Vec<String> = vec!["machine".into(), "logs".into()];
    if boot {
        argv.push("--boot".into());
    }
    if let Some(n) = tail {
        argv.push("-n".into());
        argv.push(n.clamp(1, 5000).to_string());
    }
    argv.push(name.into());
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    let bytes = run_with_timeout(&argv_ref, LIFECYCLE_TIMEOUT).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// `container machine inspect <name>` as pretty JSON (the CLI already
/// pretty-prints it; non-JSON output is passed through).
pub async fn machine_inspect(name: &str) -> Result<String> {
    reject_flag_like("machine name", name)?;
    inspect_args(&["machine", "inspect", name]).await
}

pub async fn machine_stop(name: &str) -> Result<()> {
    reject_flag_like("machine name", name)?;
    run_with_timeout(&["machine", "stop", name], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn machine_delete(name: &str) -> Result<()> {
    reject_flag_like("machine name", name)?;
    run_with_timeout(&["machine", "delete", name], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn machine_set_default(name: &str) -> Result<()> {
    reject_flag_like("machine name", name)?;
    run(&["machine", "set-default", name]).await.map(|_| ())
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RegistryLogin {
    pub hostname: String,
    pub username: String,
}

pub async fn registry_list() -> Result<Vec<RegistryLogin>> {
    let bytes = run(&["registry", "list", "--format", "json"]).await?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).context("parse `registry list` json")?;
    Ok(raw
        .into_iter()
        .map(|v| RegistryLogin {
            hostname: v
                .get("hostname")
                .or_else(|| v.get("server"))
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string(),
            username: v
                .get("username")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
        .collect())
}

/// Log in to a registry. The password travels over the child's stdin
/// (`--password-stdin`) so it never appears in an argv that `ps` or
/// crash logs could capture. The CLI stores it in the user's keychain;
/// we never persist it.
pub async fn registry_login(server: &str, username: &str, password: &str) -> Result<()> {
    reject_flag_like("registry server", server)?;
    reject_flag_like("registry username", username)?;
    let mut child = Command::new(bin())
        .args([
            "registry",
            "login",
            "--password-stdin",
            "-u",
            username,
            server,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to spawn `{} registry login`", bin()))?;
    {
        use tokio::io::AsyncWriteExt;
        let mut stdin = child.stdin.take().context("registry login stdin")?;
        stdin.write_all(password.as_bytes()).await?;
        // Drop closes the pipe so the CLI sees EOF and proceeds.
    }
    let out = tokio::time::timeout(RUN_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| {
            anyhow!(
                "`registry login` timed out after {}s",
                RUN_TIMEOUT.as_secs()
            )
        })??;
    if !out.status.success() {
        return Err(anyhow!(
            "`registry login` exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

pub async fn registry_logout(server: &str) -> Result<()> {
    reject_flag_like("registry server", server)?;
    run(&["registry", "logout", server]).await.map(|_| ())
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
// ─── Create volume / network ──────────────────────────────────────────
// `container volume create [-s SIZE] [--label k=v]... <name>` and
// `container network create [--subnet CIDR] [--subnet-v6 PREFIX]
// [--internal] [--label k=v]... <name>`. Values are validated here so the
// user gets a specific reason instead of the CLI's terse error.

/// Volume name: `^[A-Za-z0-9][A-Za-z0-9_.-]*$`, up to 255 characters (the
/// CLI accepts far longer names than containers, which cap at 63).
pub fn validate_volume_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 255
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if ok {
        Ok(())
    } else {
        Err(anyhow!(
            "volume name `{name}` must be 1-255 letters, digits, `_`, `.` or `-`, starting with a letter or digit"
        ))
    }
}

/// Network name: `^[a-z0-9](?:[a-z0-9._-]{0,61}[a-z0-9])?$` (lowercase,
/// up to 63 characters, alphanumeric at both ends).
pub fn validate_network_name(name: &str) -> Result<()> {
    let b = name.as_bytes();
    let inner =
        |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-');
    let edge = |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    let ok = !b.is_empty()
        && b.len() <= 63
        && b.first().is_some_and(edge)
        && b.last().is_some_and(edge)
        && b.iter().all(inner);
    if ok {
        Ok(())
    } else {
        Err(anyhow!(
            "network name `{name}` must be 1-63 lowercase letters, digits, `.`, `_` or `-`, starting and ending with a letter or digit"
        ))
    }
}

/// Volume size: a number with an optional K/M/G/T/P suffix (and optional
/// trailing `B`), e.g. `512M`, `10G`, `1.5G`. The CLI enforces the minimum.
pub fn validate_volume_size(size: &str) -> Result<()> {
    let s = size.trim_end_matches(['b', 'B']);
    let s = s
        .strip_suffix(['k', 'K', 'm', 'M', 'g', 'G', 't', 'T', 'p', 'P'])
        .unwrap_or(s);
    let (int, frac) = s.split_once('.').unwrap_or((s, "0"));
    let digits = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit());
    if digits(int) && digits(frac) && !size.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "volume size `{size}` must be a number with an optional K, M, G, T or P suffix, e.g. 512M or 10G"
        ))
    }
}

/// `key=value` label with a non-empty key and no whitespace in the key.
pub fn validate_label(label: &str) -> Result<()> {
    match label.split_once('=') {
        Some((k, _)) if !k.is_empty() && !k.chars().any(char::is_whitespace) => Ok(()),
        _ => Err(anyhow!("label `{label}` must be key=value")),
    }
}

/// IPv4 subnet in CIDR form (`192.168.100.0/24`).
pub fn validate_subnet_v4(cidr: &str) -> Result<()> {
    let ok = cidr.split_once('/').is_some_and(|(ip, len)| {
        ip.parse::<std::net::Ipv4Addr>().is_ok() && len.parse::<u8>().is_ok_and(|l| l <= 32)
    });
    if ok {
        Ok(())
    } else {
        Err(anyhow!(
            "subnet `{cidr}` must be an IPv4 CIDR such as 192.168.100.0/24"
        ))
    }
}

/// IPv6 prefix in CIDR form (`fd00:1234::/64`).
pub fn validate_subnet_v6(cidr: &str) -> Result<()> {
    let ok = cidr.split_once('/').is_some_and(|(ip, len)| {
        ip.parse::<std::net::Ipv6Addr>().is_ok() && len.parse::<u8>().is_ok_and(|l| l <= 128)
    });
    if ok {
        Ok(())
    } else {
        Err(anyhow!(
            "IPv6 prefix `{cidr}` must be a CIDR such as fd00:1234::/64"
        ))
    }
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCreateArgs {
    pub name: String,
    /// `-s`: e.g. `10G`. Empty/None = the CLI default (a large sparse disk).
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

pub fn volume_create_argv(args: &VolumeCreateArgs) -> Result<Vec<String>> {
    validate_volume_name(&args.name)?;
    let mut argv: Vec<String> = vec!["volume".into(), "create".into()];
    if let Some(size) = args
        .size
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        validate_volume_size(size)?;
        argv.push("-s".into());
        argv.push(size.into());
    }
    for l in args
        .labels
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
    {
        validate_label(l)?;
        argv.push("--label".into());
        argv.push(l.into());
    }
    argv.push(args.name.clone());
    Ok(argv)
}

pub async fn create_volume(args: VolumeCreateArgs) -> Result<()> {
    let argv = volume_create_argv(&args)?;
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    run_with_timeout(&argv_ref, LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkCreateArgs {
    pub name: String,
    /// `--subnet`, IPv4 CIDR.
    #[serde(default)]
    pub subnet: Option<String>,
    /// `--subnet-v6`, IPv6 prefix.
    #[serde(default)]
    pub subnet_v6: Option<String>,
    /// `--internal`: restrict to a host-only network.
    #[serde(default)]
    pub internal: bool,
    #[serde(default)]
    pub labels: Vec<String>,
}

pub fn network_create_argv(args: &NetworkCreateArgs) -> Result<Vec<String>> {
    validate_network_name(&args.name)?;
    let mut argv: Vec<String> = vec!["network".into(), "create".into()];
    let some = |o: &Option<String>| {
        o.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    if let Some(s) = some(&args.subnet) {
        validate_subnet_v4(&s)?;
        argv.push("--subnet".into());
        argv.push(s);
    }
    if let Some(s) = some(&args.subnet_v6) {
        validate_subnet_v6(&s)?;
        argv.push("--subnet-v6".into());
        argv.push(s);
    }
    if args.internal {
        argv.push("--internal".into());
    }
    for l in args
        .labels
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
    {
        validate_label(l)?;
        argv.push("--label".into());
        argv.push(l.into());
    }
    argv.push(args.name.clone());
    Ok(argv)
}

pub async fn create_network(args: NetworkCreateArgs) -> Result<()> {
    let argv = network_create_argv(&args)?;
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    run_with_timeout(&argv_ref, LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn delete_volume(name: &str) -> Result<()> {
    reject_flag_like("volume name", name)?;
    run(&["volume", "delete", name]).await.map(|_| ())
}
pub async fn delete_network(id: &str) -> Result<()> {
    reject_flag_like("network name", id)?;
    run(&["network", "delete", id]).await.map(|_| ())
}

// Open a Terminal.app window running `<runtime> exec -it <id> /bin/sh`.
// macOS-only because the host project (Apple's `container` runtime) is
// macOS-only.
pub fn exec_in_terminal(id: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    let runtime_bin = bin();
    let cmd_line = format!(
        "{} exec -it {} /bin/sh",
        shell_quote(runtime_bin),
        shell_quote(id)
    );
    let script = format!(
        r#"tell application "Terminal" to do script "{}""#,
        cmd_line.replace('\\', "\\\\").replace('"', "\\\"")
    );
    std::process::Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .spawn()
        .with_context(|| "failed to spawn osascript")?;
    let _ = std::process::Command::new("/usr/bin/osascript")
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
        .with_context(|| format!("spawn `{} {}`", bin(), mask_env_args(args).join(" ")))
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
#[path = "runtime_fixtures_test.rs"]
mod fixtures_test;

// Tests for the container 1.6/1.7 features (commit, aliases, create
// volume/network, machine options, native CPU).
#[cfg(test)]
#[path = "runtime_features_test.rs"]
mod features_test;

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
    fn shell_quote_escapes_only_when_needed() {
        assert_eq!(shell_quote("simple"), "simple");
        assert_eq!(shell_quote("path/to/bin"), "path/to/bin");
        assert_eq!(shell_quote("has space"), "'has space'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    fn words(s: &str) -> Vec<String> {
        split_command(s).unwrap()
    }

    #[test]
    fn split_command_plain_whitespace() {
        assert_eq!(
            words("node server.js --port 3000"),
            ["node", "server.js", "--port", "3000"]
        );
        assert_eq!(words("  a \t b\n c  "), ["a", "b", "c"]);
        assert!(words("").is_empty());
        assert!(words("   ").is_empty());
    }

    #[test]
    fn split_command_sh_c_with_quoted_script() {
        // The F18 case: previously split into `"echo`, `hi`, `&&`, ...
        assert_eq!(
            words(r#"sh -c "echo hi && sleep 5""#),
            ["sh", "-c", "echo hi && sleep 5"]
        );
        assert_eq!(
            words("sh -c 'echo hi && sleep 5'"),
            ["sh", "-c", "echo hi && sleep 5"]
        );
    }

    #[test]
    fn split_command_quote_rules() {
        // single quotes are literal, including backslashes and double quotes
        assert_eq!(words(r#"echo 'a "b" \n $x'"#), ["echo", r#"a "b" \n $x"#]);
        // double quotes: \" and \\ escape, other backslashes stay
        assert_eq!(
            words(r#"echo "say \"hi\" \\ \n""#),
            ["echo", r#"say "hi" \ \n"#]
        );
        // adjacent quoted/unquoted pieces join into one word
        assert_eq!(words(r#"a"b c"'d e'f"#), ["ab cd ef"]);
        // empty quotes make an empty word
        assert_eq!(words(r#"echo "" ''"#), ["echo", "", ""]);
        // backslash outside quotes escapes a space, a quote or a backslash
        assert_eq!(words(r"echo a\ b \'c \\"), ["echo", "a b", "'c", "\\"]);
        // line continuation disappears
        assert_eq!(words("a \\\nb"), ["a", "b"]);
        // a single quote inside double quotes is just a character
        assert_eq!(words(r#"echo "it's""#), ["echo", "it's"]);
    }

    #[test]
    fn split_command_unterminated_input_is_an_error() {
        for bad in [
            r#"sh -c "echo hi"#,
            "sh -c 'echo hi",
            "echo \\",
            r#"echo "abc\"#,
        ] {
            let e = split_command(bad).unwrap_err().to_string();
            assert!(
                e.contains("unterminated") || e.contains("dangling"),
                "{bad}: {e}"
            );
        }
    }

    #[test]
    fn run_image_argv_keeps_quoted_command_as_single_args() {
        let args = RunArgs {
            image: "alpine".into(),
            command: Some(r#"sh -c "echo hi && sleep 5""#.into()),
            ..Default::default()
        };
        let argv = run_image_argv(&args).unwrap();
        assert_eq!(
            &argv[argv.len() - 4..],
            ["alpine", "sh", "-c", "echo hi && sleep 5"]
        );
        let bad = RunArgs {
            image: "alpine".into(),
            command: Some("sh -c \"oops".into()),
            ..Default::default()
        };
        let e = format!("{:#}", run_image_argv(&bad).unwrap_err());
        assert!(e.contains("unterminated"), "{e}");
    }

    #[test]
    fn mask_env_args_hides_values_but_keeps_everything_else() {
        let argv = [
            "run",
            "-d",
            "--name",
            "s_web",
            "-e",
            "DB_PASS=hunter2=x",
            "-e",
            "INHERIT",
            "--env",
            "A=1",
            "--env=B=2",
            "-p",
            "80:80",
            "img",
            "sh",
            "-e",
            "not-env",
        ];
        let masked = mask_env_args(&argv);
        assert_eq!(
            masked,
            [
                "run",
                "-d",
                "--name",
                "s_web",
                "-e",
                "DB_PASS=***",
                "-e",
                "INHERIT",
                "--env",
                "A=***",
                "--env=B=***",
                "-p",
                "80:80",
                "img",
                "sh",
                "-e",
                "not-env",
            ]
        );
        assert!(!masked.join(" ").contains("hunter2"));
    }

    #[test]
    fn reject_flag_like_rejects_empty_and_dash_prefixed() {
        assert!(reject_flag_like("container id", "").is_err());
        assert!(reject_flag_like("container id", "--all").is_err());
        assert!(reject_flag_like("container id", "-f").is_err());
        assert!(reject_flag_like("container id", "web").is_ok());
        assert!(reject_flag_like("image reference", "docker.io/library/alpine:3").is_ok());
        // A dash anywhere but the front is fine.
        assert!(reject_flag_like("container id", "my-app").is_ok());
        assert!(reject_flag_like("path", "./-name").is_ok());
        let e = reject_flag_like("output path", "-x")
            .unwrap_err()
            .to_string();
        assert!(e.contains("./"), "path errors should hint at ./: {e}");
        let e = reject_flag_like("container id", "-x")
            .unwrap_err()
            .to_string();
        assert!(!e.contains("./"), "non-path errors need no hint: {e}");
    }

    // Every runtime.rs entry point taking a positional value must reject a
    // flag-like one *before* spawning anything (these run with no CLI).
    #[tokio::test]
    async fn runtime_entry_points_reject_flag_like_values() {
        let bad = "--all";
        assert!(inspect(bad).await.is_err());
        assert!(inspect_volume(bad).await.is_err());
        assert!(inspect_network(bad).await.is_err());
        assert!(inspect_image(bad).await.is_err());
        assert!(start(bad).await.is_err());
        assert!(stop(bad).await.is_err());
        assert!(kill(bad, None).await.is_err());
        assert!(kill("web", Some("-9")).await.is_err());
        assert!(delete(bad).await.is_err());
        assert!(restart(bad).await.is_err());
        assert!(clean_containers(&["ok1".into(), bad.into()]).await.is_err());
        assert!(delete_image(bad).await.is_err());
        assert!(tag_image(bad, "x:1").await.is_err());
        assert!(tag_image("x:1", bad).await.is_err());
        assert!(save_image(bad, "/tmp/o.tar").await.is_err());
        assert!(save_image("x:1", "-o").await.is_err());
        assert!(load_image(bad).await.is_err());
        assert!(copy_path(bad, "/tmp/x").await.is_err());
        assert!(copy_path("web:/etc/hosts", bad).await.is_err());
        assert!(export_container(bad, "/tmp/o.tar").await.is_err());
        assert!(export_container("web", bad).await.is_err());
        assert!(machine_create(
            "ok-machine",
            bad,
            None,
            None,
            true,
            &MachineUserOpts::default()
        )
        .await
        .is_err());
        assert!(machine_stop(bad).await.is_err());
        assert!(machine_delete(bad).await.is_err());
        assert!(machine_set_default(bad).await.is_err());
        assert!(registry_login(bad, "u", "p").await.is_err());
        assert!(registry_login("ghcr.io", bad, "p").await.is_err());
        assert!(registry_logout(bad).await.is_err());
        assert!(delete_volume(bad).await.is_err());
        assert!(delete_network(bad).await.is_err());
        assert!(exec_in_terminal(bad).is_err());
        let args = RunArgs {
            image: bad.into(),
            ..Default::default()
        };
        assert!(run_image(args).await.is_err());
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
        let n = parse_network(v, &std::collections::HashMap::new());
        assert_eq!(n.name, "default");
        assert_eq!(n.mode, "nat");
        assert_eq!(n.state, "active");
        assert_eq!(n.subnet, "192.168.64.0/24");
        assert_eq!(n.gateway, "192.168.64.1");
        assert_eq!(n.containers, 0);
    }

    #[test]
    fn parse_network_uses_ref_counts() {
        let v = json!({
            "id": "cgui-t-net",
            "configuration": { "mode": "nat", "name": "cgui-t-net" },
            "status": {}
        });
        let refs = std::collections::HashMap::from([("cgui-t-net".to_string(), 3u32)]);
        let n = parse_network(v, &refs);
        assert_eq!(n.containers, 3);
    }

    #[test]
    fn network_ref_counts_counts_only_running_attachments() {
        // Verbatim shape from `container ls --all --format json` on 1.4.1:
        // a running container's `status.networks` lists what it's actually
        // attached to; a stopped one reports `status.networks: []` even
        // though `configuration.networks` still names its intended network.
        let ls = json!([
            {
                "configuration": { "networks": [{ "network": "cgui-t-net" }] },
                "status": {
                    "state": "running",
                    "networks": [{ "network": "cgui-t-net", "hostname": "a", "ipv4Address": "10.0.0.2" }]
                }
            },
            {
                "configuration": { "networks": [{ "network": "cgui-t-net" }] },
                "status": { "state": "stopped", "networks": [] }
            },
            {
                "configuration": { "networks": [{ "network": "default" }] },
                "status": {
                    "state": "running",
                    "networks": [{ "network": "default" }]
                }
            }
        ]);
        let counts = network_ref_counts(serde_json::to_vec(&ls).unwrap().as_slice());
        // Only the one running container attached to cgui-t-net counts —
        // the stopped one's `configuration.networks` entry is not counted.
        assert_eq!(counts.get("cgui-t-net"), Some(&1));
        assert_eq!(counts.get("default"), Some(&1));
    }

    #[test]
    fn volume_ref_counts_counts_volume_mounts_only() {
        let ls = json!([
            { "configuration": { "mounts": [
                // type object with the volume name nested inside.
                { "destination": "/data", "source": "/path/volume.img",
                  "type": { "volume": { "name": "vol-a", "format": "ext4" } } },
                // non-volume mount types are skipped.
                { "destination": "/run", "source": "", "type": { "tmpfs": {} } }
            ] } },
            { "configuration": { "mounts": [
                { "destination": "/data", "source": "/other/volume.img",
                  "type": { "volume": { "name": "vol-a", "format": "ext4" } } },
                { "destination": "/host", "source": "/host", "type": { "virtiofs": {} } }
            ] } }
        ]);
        let counts = volume_ref_counts(serde_json::to_vec(&ls).unwrap().as_slice());
        assert_eq!(counts.get("vol-a"), Some(&2));
        assert_eq!(counts.len(), 1);
    }

    #[test]
    fn run_image_argv_full_surface() {
        let args = RunArgs {
            image: "alpine:latest".into(),
            name: Some("web".into()),
            ports: vec!["8080:80".into()],
            env: vec!["A=1".into(), "".into()],
            command: Some("sh -c true".into()),
            cpus: Some(2),
            memory: Some("2G".into()),
            volumes: vec!["data:/data".into()],
            labels: vec!["team=infra".into()],
            network: Some("default".into()),
            aliases: vec![],
            workdir: Some("/srv".into()),
            user: Some("1000:1000".into()),
            platform: Some("linux/arm64".into()),
            rm: true,
            rosetta: false,
        };
        let argv = run_image_argv(&args).unwrap();
        let s = argv.join(" ");
        assert!(s.starts_with("run -d "));
        for frag in [
            "--name web",
            "--cpus 2",
            "--memory 2G",
            "--network default",
            "--workdir /srv",
            "--user 1000:1000",
            "--platform linux/arm64",
            "-p 8080:80",
            "-e A=1",
            "--volume data:/data",
            "--label team=infra",
            "--rm",
        ] {
            assert!(s.contains(frag), "missing {frag} in {s}");
        }
        assert!(!s.contains("--rosetta"));
        // Image before the command tokens, command split on whitespace.
        assert!(s.ends_with("alpine:latest sh -c true"));
        // Empty env entry filtered.
        assert_eq!(argv.iter().filter(|a| *a == "-e").count(), 1);
    }

    #[test]
    fn run_image_argv_minimal() {
        let args = RunArgs {
            image: "redis:7".into(),
            ..Default::default()
        };
        assert_eq!(run_image_argv(&args).unwrap(), vec!["run", "-d", "redis:7"]);
    }

    #[test]
    fn validate_entity_name_accepts_63_chars() {
        let name = "a".repeat(63);
        assert!(validate_entity_name("container", &name).is_ok());
    }

    #[test]
    fn validate_entity_name_rejects_64_chars() {
        let name = "a".repeat(64);
        let err = validate_entity_name("container", &name)
            .unwrap_err()
            .to_string();
        assert!(err.contains("64 characters"));
        assert!(err.contains("at most 63"));
    }

    #[test]
    fn validate_entity_name_rejects_leading_underscore_dot_dash() {
        for bad in ["_foo", ".foo", "-foo"] {
            assert!(
                validate_entity_name("container", bad).is_err(),
                "expected {bad} to be rejected"
            );
        }
    }

    #[test]
    fn validate_entity_name_rejects_single_char() {
        assert!(validate_entity_name("container", "a").is_err());
    }

    #[test]
    fn validate_entity_name_rejects_disallowed_chars() {
        for bad in ["foo bar", "foo/bar", "foo@bar", "foo:bar"] {
            assert!(
                validate_entity_name("container", bad).is_err(),
                "expected {bad} to be rejected"
            );
        }
    }

    #[test]
    fn validate_entity_name_accepts_valid_names() {
        for good in ["ab", "web-1", "db_primary", "app.v2", "Container123"] {
            assert!(validate_entity_name("container", good).is_ok());
        }
    }

    const STOP_NOT_FOUND: &str = "`container stop web` exited exit status: 1: Error: internalError: \"failed to stop container\" (cause: \"notFound: \"container with ID web not found\"\")";
    const DELETE_NOT_FOUND: &str = "`container delete web` exited exit status: 1: Error: internalError: \"failed to delete container\" (cause: \"notFound: \"container with ID web not found\"\")";
    const DELETE_RUNNING: &str = "`container delete web` exited exit status: 1: Error: internalError: \"failed to delete container\" (cause: \"invalidState: \"container web is running and can not be deleted\"\")";
    const SERVICES_DOWN: &str = "`container stop web` exited exit status: 1: Error: Plugins are unavailable. Start the container system services and retry:\n\n    container system start";

    #[test]
    fn stop_errors_for_missing_or_stopped_containers_are_benign() {
        assert!(is_benign_stop_error(STOP_NOT_FOUND));
        assert!(is_benign_stop_error(
            "`container stop web` exited exit status: 1: Error: container is not running"
        ));
        assert!(is_benign_stop_error("container web already stopped"));
    }

    #[test]
    fn real_stop_errors_are_not_benign() {
        assert!(!is_benign_stop_error(SERVICES_DOWN));
        assert!(!is_benign_stop_error(
            "`container stop web` timed out after 60s"
        ));
        assert!(!is_benign_stop_error(
            "failed to spawn `container stop web`"
        ));
    }

    #[test]
    fn delete_only_tolerates_not_found() {
        assert!(is_benign_delete_error(DELETE_NOT_FOUND));
        assert!(!is_benign_delete_error(DELETE_RUNNING));
        assert!(!is_benign_delete_error(SERVICES_DOWN));
    }

    #[test]
    fn restart_stop_tolerates_only_not_running() {
        // not-found must propagate from restart's stop (the user asked to
        // restart a container that doesn't exist).
        assert!(!is_not_running_error(STOP_NOT_FOUND));
        assert!(is_not_running_error("Error: container is not running"));
        assert!(!is_not_running_error(SERVICES_DOWN));
    }

    #[test]
    fn services_down_error_is_recognised() {
        assert!(is_services_down_error(SERVICES_DOWN));
        assert!(is_services_down_error(
            "Error: Plugins are unavailable. Start the container system services and retry"
        ));
        assert!(!is_services_down_error(STOP_NOT_FOUND));
        assert!(!is_services_down_error(DELETE_RUNNING));
    }

    #[test]
    fn parse_system_running_reads_1_4_json() {
        // 1.4.1 `--format json` running body (trimmed to the relevant field).
        assert!(parse_system_running(r#"{"status":"running","host":{}}"#));
        // 1.4.1 down states.
        assert!(!parse_system_running(r#"{"status":"unregistered"}"#));
        assert!(!parse_system_running(r#"{"status":"not running"}"#));
        // No `status` field, or non-JSON (e.g. the 0.x/1.0-1.3 table), is down.
        assert!(!parse_system_running(r#"{"host":{"cpus":8}}"#));
        assert!(!parse_system_running("status             running\n"));
        assert!(!parse_system_running(""));
    }

    #[test]
    fn machine_list_parses_1_0_shape() {
        // Verbatim from `container machine list --format json` on 1.0.0.
        let json = r#"[{"status":"stopped","memory":2147483648,"default":true,"diskSize":78725120,"cpus":2,"createdDate":"2026-06-12T15:32:16Z","id":"cgui-m-test"}]"#;
        let ms: Vec<Machine> = serde_json::from_slice(json.as_bytes()).unwrap();
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].id, "cgui-m-test");
        assert_eq!(ms[0].status, "stopped");
        assert_eq!(ms[0].cpus, 2);
        assert_eq!(ms[0].memory, 2147483648);
        assert!(ms[0].is_default);
    }

    #[test]
    fn build_argv_full_surface() {
        let args = BuildArgs {
            context: "/proj".into(),
            tag: Some("app:dev".into()),
            dockerfile: Some("/proj/Dockerfile.dev".into()),
            build_args: vec!["V=1".into()],
            target: Some("runtime".into()),
            no_cache: true,
        };
        let s = build_argv(&args).join(" ");
        assert!(s.starts_with("build --progress plain"));
        for frag in [
            "-t app:dev",
            "-f /proj/Dockerfile.dev",
            "--build-arg V=1",
            "--target runtime",
            "--no-cache",
        ] {
            assert!(s.contains(frag), "missing {frag} in {s}");
        }
        assert!(s.ends_with(" /proj"));
    }

    #[test]
    fn clean_argv_builds_subcommand_plus_ids() {
        let ids = vec!["abc123".to_string(), "def456".to_string()];
        assert_eq!(clean_argv(&ids), vec!["clean", "abc123", "def456"]);
    }

    #[test]
    fn clean_argv_handles_single_id() {
        let ids = vec!["abc123".to_string()];
        assert_eq!(clean_argv(&ids), vec!["clean", "abc123"]);
    }

    #[test]
    fn map_clean_unsupported_translates_exit_64() {
        // Real shape from an old CLI's plugin dispatcher (verified against
        // 1.4.1 with a bogus subcommand, which fails the same way).
        let e = anyhow!(
            "`container clean abc123` exited exit status: 64: Error: Plugin 'container-clean' not found."
        );
        let mapped = map_clean_unsupported(e);
        assert_eq!(
            mapped.to_string(),
            "`container clean` requires Apple container 1.4 or later"
        );
    }

    #[test]
    fn map_clean_unsupported_passes_through_other_failures() {
        // e.g. target container isn't running, or doesn't exist — exit 1,
        // not the plugin-dispatch 64. Should reach the caller unchanged.
        let e = anyhow!(
            "`container clean abc123` exited exit status: 1: Error: internalError: \"failed to clean container\" (cause: \"notFound: \\\"container with ID abc123 not found\\\"\")"
        );
        let msg = e.to_string();
        let mapped = map_clean_unsupported(e);
        assert_eq!(mapped.to_string(), msg);

        // A usage error also exits 64 but isn't a missing subcommand.
        let usage =
            anyhow!("`container clean` exited exit status: 64: Error: Missing expected argument");
        let msg = usage.to_string();
        assert_eq!(map_clean_unsupported(usage).to_string(), msg);
    }
    fn container_json(id: &str) -> Value {
        json!({
            "status": { "state": "running" },
            "configuration": {
                "id": id,
                "image": { "reference": "docker.io/library/alpine:latest" }
            }
        })
    }

    #[test]
    fn parse_container_keeps_full_id_for_uuid_and_stack_names() {
        for id in [
            "d5d88c0b-3037-4d11-bac4-4ed8d55c4461",
            "mystack_web",
            "cgui-live-ids-12345",
        ] {
            let c = parse_container(container_json(id));
            // The CLI matches ids exactly: id must be passed through intact.
            assert_eq!(c.id, id);
            assert_eq!(c.short_id, id.chars().take(8).collect::<String>());
            assert_eq!(c.short_id.len(), 8);
        }
        // Ids of 8 chars or fewer are unchanged in both fields.
        let c = parse_container(container_json("web"));
        assert_eq!((c.id.as_str(), c.short_id.as_str()), ("web", "web"));
    }

    #[test]
    fn match_stat_is_exact_not_prefix() {
        let stat = |id: &str| StatRow {
            id: id.into(),
            ..Default::default()
        };
        let stats = vec![stat("stack_web"), stat("stack_worker")];
        assert_eq!(
            match_stat(&stats, "stack_worker").unwrap().id,
            "stack_worker"
        );
        assert_eq!(match_stat(&stats, "stack_web").unwrap().id, "stack_web");
        assert!(match_stat(&stats, "stack_").is_none());
        assert!(match_stat(&stats, "stack_web_extra").is_none());
    }

    const IMAGE_LS_ONE: &str = r#"[{"configuration":{"name":"docker.io/library/alpine:latest","descriptor":{"digest":"sha256:0123456789abcdef"}},"variants":[{"size":1073741824}]}]"#;

    #[test]
    fn image_ls_partial_output_returns_images_and_warning() {
        let stderr =
            "Warning: skipping image foo: bad manifest\nError: failed to read 2 image(s)\n";
        let l = parse_image_ls(false, IMAGE_LS_ONE.as_bytes(), stderr, &"exit status: 1").unwrap();
        assert_eq!(l.images.len(), 1);
        assert_eq!(l.images[0].reference, "docker.io/library/alpine:latest");
        let w = l.warning.unwrap();
        assert!(w.starts_with("2 images couldn't be read: "), "{w}");
        assert!(w.contains("bad manifest"));
    }

    #[test]
    fn image_ls_partial_singular_and_unknown_count() {
        let l = parse_image_ls(false, b"[]", "failed to read 1 image(s)", &"x").unwrap();
        assert!(l.images.is_empty());
        assert!(l.warning.unwrap().starts_with("1 image couldn't be read"));
        let l = parse_image_ls(false, b"[]", "", &"x").unwrap();
        assert_eq!(l.warning.as_deref(), Some("Some images couldn't be read"));
    }

    #[test]
    fn image_ls_nonzero_with_garbage_is_an_error() {
        let e = parse_image_ls(false, b"", SERVICES_DOWN, &"exit status: 1").unwrap_err();
        assert!(is_services_down_error(&e.to_string()));
        assert!(parse_image_ls(false, b"not json", "boom", &"x").is_err());
    }

    #[test]
    fn image_ls_success_has_no_warning() {
        let l = parse_image_ls(true, IMAGE_LS_ONE.as_bytes(), "", &"ok").unwrap();
        assert_eq!(l.images.len(), 1);
        assert!(l.warning.is_none());
        assert!(parse_image_ls(true, b"nope", "", &"ok").is_err());
    }

    #[test]
    fn services_down_matches_1_6_plugin_text() {
        // Verbatim 1.6+/1.7 text for every subcommand incl. build/builder.
        for sub in ["build", "builder status", "image ls"] {
            let msg = format!(
                "`container {sub}` exited exit status: 64: Error: Plugins are unavailable. Start the container system services and retry:\n\n    container system start"
            );
            assert!(is_services_down_error(&msg), "{sub}");
        }
        assert!(is_services_down_error("Plugins are unavailable."));
    }

    #[test]
    fn map_clean_unsupported_handles_1_5_wording() {
        let e =
            anyhow!("`container clean abc` exited exit status: 64: Error: unknown command 'clean'");
        assert_eq!(
            map_clean_unsupported(e).to_string(),
            "`container clean` requires Apple container 1.4 or later"
        );
        // Wording without exit 64 is not remapped.
        let e = anyhow!("`container clean abc` exited exit status: 1: unknown command 'clean'");
        assert!(map_clean_unsupported(e)
            .to_string()
            .contains("exit status: 1"));
    }

    #[test]
    fn parse_system_versions_reads_client_and_server() {
        let body =
            r#"{"client":{"version":"1.7.1"},"server":{"version":"1.4.1"},"status":"running"}"#;
        assert_eq!(
            parse_system_versions(body),
            (Some("1.7.1".into()), Some("1.4.1".into()))
        );
        assert_eq!(
            parse_system_versions("{\"status\":\"running\"}"),
            (None, None)
        );
        assert_eq!(parse_system_versions("table output"), (None, None));
    }
}
