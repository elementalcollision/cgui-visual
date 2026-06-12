// Tauri commands. Each one tries the real runtime; on failure (binary missing,
// non-zero exit, parse error) falls back to fixtures so dev still works on
// machines without Apple's container CLI.

use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::doctor as doctor_mod;
use crate::fixtures;
use crate::model::*;
use crate::prefs::Prefs;
use crate::runtime;
use crate::state::History;

/// Pick between a fixture (debug builds) and an empty default
/// (release builds). The fixture data is great for design work in
/// `cargo tauri dev`, but in shipped releases it has been leaking
/// into users' UIs — a fresh install with `container` running but
/// zero stacks ended up showing fictional MLPerf demo data, which
/// looks broken. Gating on `debug_assertions` strips the fixture
/// paths from release binaries cleanly.
fn dev_fixture_or<T>(fixture: impl FnOnce() -> T, empty: impl FnOnce() -> T) -> T {
    if cfg!(debug_assertions) {
        fixture()
    } else {
        empty()
    }
}

// Generic helper: run real impl when CLI is available, else fall back
// to the dev fixture (or empty in release).
async fn real_or_fixture<T, F, Fut>(
    real: F,
    fixture: impl FnOnce() -> T,
    empty: impl FnOnce() -> T,
) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    if !runtime::available().await {
        return dev_fixture_or(fixture, empty);
    }
    match real().await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("runtime call failed: {e:#}");
            dev_fixture_or(fixture, empty)
        }
    }
}

// ─── Lists ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_containers(history: State<'_, Arc<History>>) -> Result<Vec<Container>, String> {
    if !runtime::available().await {
        return Ok(dev_fixture_or(fixtures::containers, Vec::new));
    }
    match crate::state::poll_once(&history).await {
        Ok(cs) => Ok(cs),
        Err(e) => {
            eprintln!("list_containers poll failed: {e:#}");
            Ok(dev_fixture_or(fixtures::containers, Vec::new))
        }
    }
}

#[tauri::command]
pub async fn list_images() -> Vec<Image> {
    real_or_fixture(runtime::list_images, fixtures::images, Vec::new).await
}

#[tauri::command]
pub async fn list_volumes() -> Vec<Volume> {
    real_or_fixture(runtime::list_volumes, fixtures::volumes, Vec::new).await
}

#[tauri::command]
pub async fn list_networks() -> Vec<Network> {
    real_or_fixture(runtime::list_networks, fixtures::networks, Vec::new).await
}

#[tauri::command]
pub async fn list_stacks() -> Vec<Stack> {
    let real = crate::stacks::list_stacks().await;
    if !real.is_empty() {
        return real;
    }
    // Empty list from the real loader: in dev show the fictional MLPerf
    // stacks so designers have something to look at; in release return
    // an empty list so users see the genuine empty state instead of
    // four invented stacks they didn't create.
    dev_fixture_or(fixtures::stacks, Vec::new)
}

#[tauri::command]
pub async fn stack_up(name: String) -> Result<Vec<String>, String> {
    crate::stacks::stack_up(&name).await.map_err(err_str)
}

#[tauri::command]
pub async fn stack_down(name: String) -> Result<Vec<String>, String> {
    crate::stacks::stack_down(&name).await.map_err(err_str)
}

// (service_name, "healthy" | "unhealthy" | "—" | "unsupported:<kind>")
#[tauri::command]
pub async fn stack_health(name: String) -> Result<Vec<(String, String)>, String> {
    crate::stacks::health_check_stack(&name)
        .await
        .map_err(err_str)
}

#[tauri::command]
pub async fn inspect_container(id: String) -> String {
    if !runtime::available().await {
        // Release: valid empty JSON so parseInspect doesn't crash on
        // the frontend; dev: rich demo payload for design work.
        return dev_fixture_or(fixtures::inspect_json, || "{}".to_string());
    }
    runtime::inspect(&id).await.unwrap_or_else(|e| {
        eprintln!("inspect failed: {e:#}");
        dev_fixture_or(fixtures::inspect_json, || "{}".to_string())
    })
}

#[tauri::command]
pub async fn inspect_volume(name: String) -> Result<String, String> {
    runtime::inspect_volume(&name).await.map_err(err_str)
}

#[tauri::command]
pub async fn inspect_network(id: String) -> Result<String, String> {
    runtime::inspect_network(&id).await.map_err(err_str)
}

#[tauri::command]
pub async fn inspect_image(reference: String) -> Result<String, String> {
    runtime::inspect_image(&reference).await.map_err(err_str)
}

#[tauri::command]
pub async fn run_image(args: runtime::RunArgs) -> Result<String, String> {
    runtime::run_image(args).await.map_err(err_str)
}

#[tauri::command]
pub async fn tag_image(source: String, target: String) -> Result<(), String> {
    runtime::tag_image(&source, &target).await.map_err(err_str)
}

// ─── Prune + disk usage (1.0 parity) ──────────────────────────────────
// Each returns the CLI's stdout summary so the UI can toast what was
// reclaimed. Confirmation happens in the frontend before the call.

#[tauri::command]
pub async fn prune_containers() -> Result<String, String> {
    runtime::prune_containers().await.map_err(err_str)
}

#[tauri::command]
pub async fn prune_images() -> Result<String, String> {
    runtime::prune_images().await.map_err(err_str)
}

#[tauri::command]
pub async fn prune_volumes() -> Result<String, String> {
    runtime::prune_volumes().await.map_err(err_str)
}

#[tauri::command]
pub async fn prune_networks() -> Result<String, String> {
    runtime::prune_networks().await.map_err(err_str)
}

#[tauri::command]
pub async fn system_df() -> Result<runtime::DiskUsage, String> {
    runtime::system_df().await.map_err(err_str)
}

// Probe used by first-run onboarding. Frontend calls this on mount and
// shows the OnboardingModal when it returns false. Cheap (single fork +
// `container --version`), so re-running on a timer is fine.
#[tauri::command]
pub async fn runtime_available() -> bool {
    runtime::available().await
}

// ─── Long-form metric history (B6) ────────────────────────────────────
//
// Returns persisted CPU/mem/net/disk samples for one container within
// the last `since_secs` seconds. Empty when the sidecar DB hasn't
// recorded any rows for this id (yet) or init failed.

#[tauri::command]
pub fn container_history(id: String, since_secs: i64) -> Vec<crate::history::HistoryPoint> {
    crate::history::load(&id, since_secs)
}

// ─── Embedded terminal (B5) ───────────────────────────────────────────
//
// Open a pty that runs `<runtime> exec -it <id> <shell>` and stream the
// output back to the frontend via `pty:tick:<session_id>` events. The
// session id round-trips so the UI can multiplex stdin writes + resize
// + close.

#[tauri::command]
pub fn pty_open(
    app: AppHandle,
    id: String,
    shell: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<String, String> {
    let runtime_bin = runtime::current_bin();
    crate::pty::open(
        &app,
        &runtime_bin,
        &id,
        shell.as_deref(),
        cols.max(2),
        rows.max(2),
    )
    .map_err(err_str)
}

#[tauri::command]
pub fn pty_write(session_id: String, data: String) -> Result<(), String> {
    crate::pty::write(&session_id, &data).map_err(err_str)
}

#[tauri::command]
pub fn pty_resize(session_id: String, cols: u16, rows: u16) -> Result<(), String> {
    crate::pty::resize(&session_id, cols.max(2), rows.max(2)).map_err(err_str)
}

#[tauri::command]
pub fn pty_close(session_id: String) -> Result<(), String> {
    crate::pty::close(&session_id).map_err(err_str)
}

// Per-binary availability probe (B8). Settings calls this once per
// runtime option to render an availability badge so users can see at a
// glance which runtimes are installed before flipping the active one.
#[tauri::command]
pub async fn probe_runtime(name: String) -> bool {
    // Only allow probing the runtimes the UI actually offers, to avoid
    // turning this into a generic "is this binary on PATH" oracle.
    if !matches!(name.as_str(), "container" | "docker" | "podman") {
        return false;
    }
    runtime::probe_bin(&name).await
}

// ─── Stack snapshot / restore (B11) ───────────────────────────────────
//
// Snapshot scope is the stack *configuration* only: the TOML body
// wrapped in a self-describing JSON envelope. Volume *data* is out of
// scope — see snapshot.rs for the rationale. The frontend writes the
// returned string to a file via the browser's download flow; restore
// is the inverse and accepts the JSON contents directly.

#[tauri::command]
pub fn snapshot_stack(name: String, note: Option<String>) -> Result<String, String> {
    crate::snapshot::create(&name, note.as_deref().unwrap_or("")).map_err(err_str)
}

#[tauri::command]
pub fn restore_stack(json: String, overwrite: bool) -> Result<String, String> {
    crate::snapshot::restore(&json, overwrite).map_err(err_str)
}

// Convenience: restore by reading the file ourselves so the frontend
// only needs the picked path (no fs plugin required). Mirrors the
// import_compose UX.
#[tauri::command]
pub fn restore_stack_from_path(path: String, overwrite: bool) -> Result<String, String> {
    let json = std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))?;
    crate::snapshot::restore(&json, overwrite).map_err(err_str)
}

// Render a stack as a docker-compose.yml string. Round-trips with
// import_compose: convert(parse(export(s))) ≈ s for any importable
// stack. The frontend writes the returned string to a file via the
// browser's download flow, so we don't touch the filesystem here.
#[tauri::command]
pub fn export_compose(name: String) -> Result<String, String> {
    crate::compose::export_named(&name).map_err(err_str)
}

// Convert a docker-compose.yml at `path` into a cgui stack TOML and write
// it to ~/.config/cgui/stacks/<name>.toml. Returns the destination path
// so the frontend can show it in a toast.
//
// Refuses to overwrite an existing stack with the same name unless
// `overwrite` is true — Compose imports usually happen against fresh
// stack names, but accidental clobbers would silently lose hand-edits.
#[tauri::command]
pub async fn import_compose(path: String, overwrite: bool) -> Result<String, String> {
    use std::path::PathBuf;

    let p = PathBuf::from(&path);
    let (stack, toml_text) = crate::compose::import_file(&p).map_err(err_str)?;

    let dir = crate::stacks::stacks_dir()
        .ok_or_else(|| "could not resolve $XDG_CONFIG_HOME or $HOME".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create stacks dir: {e}"))?;
    let dest = dir.join(format!("{}.toml", stack.name));

    if dest.exists() && !overwrite {
        return Err(format!(
            "stack '{}' already exists at {}; pass overwrite=true to replace",
            stack.name,
            dest.display()
        ));
    }
    std::fs::write(&dest, toml_text).map_err(|e| format!("write {}: {e}", dest.display()))?;
    Ok(dest.display().to_string())
}

// ─── Actions (return Result so the frontend can surface errors) ───────

#[tauri::command]
pub async fn start_container(id: String) -> Result<(), String> {
    runtime::start(&id).await.map_err(err_str)
}
#[tauri::command]
pub async fn stop_container(id: String) -> Result<(), String> {
    runtime::stop(&id).await.map_err(err_str)
}
#[tauri::command]
pub async fn kill_container(id: String, signal: Option<String>) -> Result<(), String> {
    runtime::kill(&id, signal.as_deref()).await.map_err(err_str)
}
#[tauri::command]
pub async fn delete_container(id: String) -> Result<(), String> {
    runtime::delete(&id).await.map_err(err_str)
}
#[tauri::command]
pub async fn restart_container(id: String) -> Result<(), String> {
    runtime::restart(&id).await.map_err(err_str)
}
#[tauri::command]
pub async fn delete_image(reference: String) -> Result<(), String> {
    runtime::delete_image(&reference).await.map_err(err_str)
}
#[tauri::command]
pub async fn delete_volume(name: String) -> Result<(), String> {
    runtime::delete_volume(&name).await.map_err(err_str)
}
#[tauri::command]
pub async fn delete_network(id: String) -> Result<(), String> {
    runtime::delete_network(&id).await.map_err(err_str)
}
#[tauri::command]
pub fn exec_container(id: String) -> Result<(), String> {
    runtime::exec_in_terminal(&id).map_err(err_str)
}

fn err_str(e: anyhow::Error) -> String {
    format!("{e:#}")
}

// ─── Streaming logs ───────────────────────────────────────────────────
//
// Frontend invokes `start_log_stream(id)`; backend spawns `container logs -f`
// and emits one `logs:tick` event per line. `stop_log_stream` aborts the
// child via kill_on_drop.

#[tauri::command]
pub async fn start_log_stream(
    app: AppHandle,
    id: String,
    boot: Option<bool>,
    tail: Option<u32>,
) -> Result<(), String> {
    if !runtime::available().await {
        // Dev: replay fixtures with a small delay so the LogsView still
        // gets data for design work. Release: no-op — emitting fake
        // log lines into a user's view would be confusing.
        if cfg!(debug_assertions) {
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                for line in fixtures::logs() {
                    let _ = app2.emit("logs:tick", line);
                    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                }
            });
        }
        return Ok(());
    }
    // `--boot` swaps stdio for the VM boot log (1.0); `-n` tails the
    // last N lines instead of replaying the whole history on attach.
    let mut argv: Vec<String> = vec!["logs".into(), "-f".into()];
    if boot.unwrap_or(false) {
        argv.push("--boot".into());
    }
    let tail_str;
    if let Some(n) = tail.filter(|n| *n > 0) {
        tail_str = n.to_string();
        argv.push("-n".into());
        argv.push(tail_str);
    }
    argv.push(id.clone());
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    let child = runtime::spawn(&argv_ref).map_err(err_str)?;
    let app2 = app.clone();
    let id_for_persist = id.clone();
    // Boot logs are VM console output, not container stdio — don't mix
    // them into the per-container log history sidecar.
    let persist = !boot.unwrap_or(false);
    tauri::async_runtime::spawn(async move {
        let _ = runtime::drain_lines(child, move |line| {
            // Persist before emitting so a crash mid-tick still gets the
            // line on disk. record_log no-ops when the DB isn't init'd.
            if persist {
                crate::history::record_log(&id_for_persist, &line);
            }
            let _ = app2.emit("logs:tick", line);
        })
        .await;
        let _ = app.emit("logs:tick", "[follow ended]".to_string());
    });
    Ok(())
}

// Retrospective log view (B12). Returns the last `limit` persisted
// lines for a container, optionally filtered by substring. Empty when
// the sidecar DB hasn't recorded any lines yet.
#[tauri::command]
pub fn load_logs(
    container_id: String,
    limit: Option<i64>,
    query: Option<String>,
) -> Vec<crate::history::LogLine> {
    crate::history::load_logs(&container_id, limit.unwrap_or(500), query.as_deref())
}

// ─── Streaming pull ───────────────────────────────────────────────────

#[tauri::command]
pub async fn start_pull(app: AppHandle, reference: String) -> Result<(), String> {
    if !runtime::available().await {
        // Dev-only fake stream so designers can see the progress UI;
        // release returns an error so the modal surfaces a real reason
        // instead of pretending to pull through to "done".
        if cfg!(debug_assertions) {
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                for line in fixtures::pull_stream() {
                    let _ = app2.emit("pull:tick", line);
                    tokio::time::sleep(std::time::Duration::from_millis(380)).await;
                }
                let _ = app2.emit("pull:done", true);
            });
            return Ok(());
        }
        return Err("container runtime is not available".into());
    }
    let mut argv = vec!["image", "pull", "--progress=plain"];
    argv.push(&reference);
    let child = runtime::spawn(&argv).map_err(err_str)?;
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let res = runtime::drain_lines(child, move |line| {
            let _ = app2.emit("pull:tick", line);
        })
        .await;
        let ok = matches!(res, Ok(s) if s.success());
        let _ = app.emit("pull:done", ok);
    });
    Ok(())
}

// ─── Doctor ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn doctor() -> Vec<DoctorCheck> {
    doctor_mod::run().await
}

// ─── Trivy + updates (kept on fixtures for now) ───────────────────────

#[tauri::command]
pub async fn scan_image(image: String) -> TrivyResult {
    let result = crate::trivy::scan(&image).await.unwrap_or_else(|| {
        // Either trivy isn't on PATH or the scan failed. In dev show the
        // demo findings so the modal isn't blank. In release return an
        // empty result so users don't see invented CVEs against their
        // image — Doctor already flags the missing trivy binary.
        dev_fixture_or(
            || {
                let mut fb = fixtures::trivy();
                fb.image = image.clone();
                fb
            },
            || TrivyResult {
                image: image.clone(),
                counts: serde_json::json!({
                    "CRITICAL": 0, "HIGH": 0, "MEDIUM": 0, "LOW": 0,
                }),
                findings: vec![],
            },
        )
    });
    // Persist a scan summary for the trend strip + new-CVEs diff (B7).
    // Best-effort — record_scan no-ops if the DB isn't initialised.
    crate::history::record_scan(&result.image, &result.findings);
    result
}

// Trivy scan history for one image (B7). Returns the most-recent-first
// list of scan summaries plus the CVEs that appeared in the latest scan
// but not the previous one. Empty when no scans are recorded for image.
#[tauri::command]
pub fn vuln_history(image: String, limit: Option<i64>) -> crate::history::VulnHistory {
    crate::history::load_scans(&image, limit.unwrap_or(60))
}

#[tauri::command]
pub async fn list_updates() -> Vec<Update> {
    let real = crate::updates::check().await;
    if real.is_empty() {
        vec![]
    } else {
        real
    }
}

// Kept for backward-compat with the frontend api.ts; new code should
// use start_pull + pull:tick / start_log_stream + logs:tick. Dev-only
// fixture stream so designers can preview the panel; release returns
// empty so users don't see invented log spam against their containers.
#[tauri::command]
pub fn pull_stream() -> Vec<String> {
    dev_fixture_or(fixtures::pull_stream, Vec::new)
}
#[tauri::command]
pub fn tail_logs(_target: Option<String>) -> Vec<String> {
    dev_fixture_or(fixtures::logs, Vec::new)
}

// ─── Prefs ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn load_prefs() -> Prefs {
    Prefs::load()
}

#[tauri::command]
pub fn save_prefs(prefs: Prefs) -> Result<(), String> {
    runtime::set_bin(&prefs.runtime);
    prefs.save().map_err(err_str)
}
