use super::*;

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

#[cfg(test)]
mod tests {
    use super::*;

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
