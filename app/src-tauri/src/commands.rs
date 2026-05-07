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

// Generic helper: run real impl when CLI is available, else fixture fallback.
async fn real_or_fixture<T, F, Fut>(real: F, fallback: T) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    if !runtime::available().await {
        return fallback;
    }
    match real().await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("runtime call failed: {e:#}");
            fallback
        }
    }
}

// ─── Lists ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_containers(history: State<'_, Arc<History>>) -> Result<Vec<Container>, String> {
    if !runtime::available().await {
        return Ok(fixtures::containers());
    }
    match crate::state::poll_once(&history).await {
        Ok(cs) => Ok(cs),
        Err(e) => {
            eprintln!("list_containers poll failed: {e:#}");
            Ok(fixtures::containers())
        }
    }
}

#[tauri::command]
pub async fn list_images() -> Vec<Image> {
    real_or_fixture(runtime::list_images, fixtures::images()).await
}

#[tauri::command]
pub async fn list_volumes() -> Vec<Volume> {
    real_or_fixture(runtime::list_volumes, fixtures::volumes()).await
}

#[tauri::command]
pub async fn list_networks() -> Vec<Network> {
    real_or_fixture(runtime::list_networks, fixtures::networks()).await
}

#[tauri::command]
pub async fn list_stacks() -> Vec<Stack> {
    let real = crate::stacks::list_stacks().await;
    if real.is_empty() {
        fixtures::stacks()
    } else {
        real
    }
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
        return fixtures::inspect_json();
    }
    runtime::inspect(&id).await.unwrap_or_else(|e| {
        eprintln!("inspect failed: {e:#}");
        fixtures::inspect_json()
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

// Probe used by first-run onboarding. Frontend calls this on mount and
// shows the OnboardingModal when it returns false. Cheap (single fork +
// `container --version`), so re-running on a timer is fine.
#[tauri::command]
pub async fn runtime_available() -> bool {
    runtime::available().await
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
pub async fn kill_container(id: String) -> Result<(), String> {
    runtime::kill(&id).await.map_err(err_str)
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
pub async fn start_log_stream(app: AppHandle, id: String) -> Result<(), String> {
    if !runtime::available().await {
        // Replay fixtures with a small delay so the LogsView still gets data.
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            for line in fixtures::logs() {
                let _ = app2.emit("logs:tick", line);
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            }
        });
        return Ok(());
    }
    let child = runtime::spawn(&["logs", "-f", &id]).map_err(err_str)?;
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = runtime::drain_lines(child, move |line| {
            let _ = app2.emit("logs:tick", line);
        })
        .await;
        let _ = app.emit("logs:tick", "[follow ended]".to_string());
    });
    Ok(())
}

// ─── Streaming pull ───────────────────────────────────────────────────

#[tauri::command]
pub async fn start_pull(app: AppHandle, reference: String) -> Result<(), String> {
    if !runtime::available().await {
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
    crate::trivy::scan(&image).await.unwrap_or_else(|| {
        // Either trivy isn't on PATH or the scan failed; surface fixtures so
        // the modal still has something to show. Doctor already flags the
        // missing binary.
        let mut fb = fixtures::trivy();
        fb.image = image;
        fb
    })
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

// Kept for backward-compat with the frontend api.ts; just returns the fixture
// pull stream synchronously. New code should use start_pull + pull:tick.
#[tauri::command]
pub fn pull_stream() -> Vec<String> {
    fixtures::pull_stream()
}
#[tauri::command]
pub fn tail_logs(_target: Option<String>) -> Vec<String> {
    fixtures::logs()
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
