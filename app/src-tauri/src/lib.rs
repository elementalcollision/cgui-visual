mod commands;
pub mod compose;
mod doctor;
mod fixtures;
pub mod history;
pub mod model;
mod prefs;
pub mod pty;
pub mod runtime;
pub mod snapshot;
pub mod stacks;
pub mod state;
pub mod trivy;
pub mod updates;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_notification::NotificationExt;

// Icon embedded at compile time so the tray works in dev + bundled builds.
const TRAY_ICON_PNG: &[u8] = include_bytes!("../icons/32x32.png");

// Format the running container count for the tray title. Empty string when
// the runtime isn't reachable so we don't show a stale 0.
fn fmt_tray_title(running: Option<usize>) -> String {
    match running {
        Some(n) => format!(" {n}"),
        None => String::new(),
    }
}

// Tracks the currently-registered global hotkey so we can unregister it
// before re-registering a new one (or when the user clears the setting).
// `Mutex<Option<String>>` since access happens both from setup and from a
// frontend command on the same thread.
static CURRENT_HOTKEY: Mutex<Option<String>> = Mutex::new(None);

fn parse_shortcut(
    accelerator: &str,
) -> Result<tauri_plugin_global_shortcut::Shortcut, Box<dyn std::error::Error>> {
    accelerator
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| format!("invalid accelerator '{accelerator}': {e:?}").into())
}

fn focus_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn register_summon_hotkey<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    accelerator: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    let gs = app.global_shortcut();
    // Tear down any previously-registered summon shortcut first.
    if let Ok(mut guard) = CURRENT_HOTKEY.lock() {
        if let Some(prev) = guard.take() {
            if let Ok(prev_sc) = parse_shortcut(&prev) {
                let _ = gs.unregister(prev_sc);
            }
        }
    }

    let sc = parse_shortcut(accelerator)?;
    let app_handle = app.clone();
    gs.on_shortcut(sc, move |_app, _shortcut, event| {
        // Only react on key-press; tauri emits both Pressed and Released.
        if event.state == ShortcutState::Pressed {
            focus_main_window(&app_handle);
        }
    })?;

    if let Ok(mut guard) = CURRENT_HOTKEY.lock() {
        *guard = Some(accelerator.to_string());
    }
    Ok(())
}

fn unregister_summon_hotkey<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let gs = app.global_shortcut();
    if let Ok(mut guard) = CURRENT_HOTKEY.lock() {
        if let Some(prev) = guard.take() {
            if let Ok(prev_sc) = parse_shortcut(&prev) {
                let _ = gs.unregister(prev_sc);
            }
        }
    }
}

// Frontend Settings panel calls this whenever the hotkey field changes.
// Empty string clears the binding. Returns Err with a human-readable
// message that the UI can show via toast. Lives in a child module so the
// `#[tauri::command]`-generated symbol doesn't collide with the symbols
// produced by `generate_handler!` at the crate root.
pub mod hotkey_cmd {
    #[tauri::command]
    pub fn set_global_hotkey(app: tauri::AppHandle, accelerator: String) -> Result<(), String> {
        if accelerator.trim().is_empty() {
            super::unregister_summon_hotkey(&app);
            return Ok(());
        }
        super::register_summon_hotkey(&app, accelerator.trim()).map_err(|e| format!("{e}"))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // Persist + restore window size, position, maximised state, and
        // fullscreen across sessions. Stored at ~/.config/cgui-gui (same
        // dir as the prefs JSON via Tauri's default scope).
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .manage(Arc::new(state::History::new()))
        .setup(|app| {
            // Patch PATH first so every subsequent runtime probe / child
            // spawn (container, docker, podman, trivy) can find binaries
            // installed at /usr/local/bin or /opt/homebrew/bin even when
            // the app was launched from Finder / Spotlight / Dock with
            // the bare launchd PATH.
            runtime::ensure_user_path();

            // Load prefs once: seed runtime + register optional global hotkey.
            let initial_prefs = prefs::Prefs::load();
            runtime::set_bin(&initial_prefs.runtime);

            // Open the long-form history sidecar (B6). Failures only mean
            // the trends tab will be empty; the live UI keeps working.
            if let Err(e) = history::init() {
                eprintln!("history::init failed (trends will be empty): {e:#}");
            }

            // Register the global summon hotkey if one is configured. Failures
            // (bad accelerator string, hotkey already taken by another app) are
            // logged but non-fatal — the user can fix it from Settings.
            if !initial_prefs.global_hotkey.is_empty() {
                if let Err(e) = register_summon_hotkey(app.handle(), &initial_prefs.global_hotkey) {
                    eprintln!(
                        "global hotkey '{}' failed to register: {e:#}",
                        initial_prefs.global_hotkey
                    );
                }
            }

            // Menubar tray: icon + running-count title + minimal menu.
            // On left click we toggle the main window (show + focus, or hide
            // if already foregrounded). The "Show" menu item does the same.
            let tray_icon = Image::from_bytes(TRAY_ICON_PNG)?;
            let show_item = MenuItem::with_id(app, "show", "Show cgui", true, None::<&str>)?;
            let hide_item = MenuItem::with_id(app, "hide", "Hide cgui", true, None::<&str>)?;
            let count_item = MenuItem::with_id(app, "count", "Containers: —", false, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit cgui", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &show_item,
                    &hide_item,
                    &PredefinedMenuItem::separator(app)?,
                    &count_item,
                    &PredefinedMenuItem::separator(app)?,
                    &quit_item,
                ],
            )?;
            let _tray = TrayIconBuilder::with_id("main")
                .icon(tray_icon)
                .icon_as_template(true)
                .title(fmt_tray_title(None))
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            // Toggle: bring to front if hidden/minimized, else hide.
                            let visible = w.is_visible().unwrap_or(false);
                            let focused = w.is_focused().unwrap_or(false);
                            if visible && focused {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            let handle = app.handle().clone();
            let history: Arc<state::History> = app.state::<Arc<state::History>>().inner().clone();
            let count_item_for_tick = count_item.clone();
            // Per-id (status, exit_code) seen on the previous tick. Powers
            // B10 exit-notification diffing without leaking memory: any id
            // not present in the latest poll is dropped at the bottom.
            let mut prev_state: HashMap<String, (String, Option<i32>)> = HashMap::new();
            tauri::async_runtime::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(2));
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    tick.tick().await;
                    if !runtime::available().await {
                        if let Some(t) = handle.tray_by_id("main") {
                            let _ = t.set_title(Some(fmt_tray_title(None)));
                        }
                        continue;
                    }
                    match state::poll_once(&history).await {
                        Ok(cs) => {
                            // Persist this tick before any other side effects
                            // so a crash mid-tick still gets the data on disk.
                            crate::history::record(&cs);
                            let running = cs
                                .iter()
                                .filter(|c| c.status.eq_ignore_ascii_case("running"))
                                .count();
                            if let Some(t) = handle.tray_by_id("main") {
                                let _ = t.set_title(Some(fmt_tray_title(Some(running))));
                            }
                            let _ = count_item_for_tick
                                .set_text(format!("Containers: {running} running"));

                            // B10: notify on running→exited transitions. Only
                            // fire when prefs.notify_on_exit is on, the
                            // previous status was running (so we don't blast a
                            // notification on first tick), and the new status
                            // is exited/stopped/dead. Non-zero exit codes get
                            // explicit highlight in the body.
                            if prefs::Prefs::load().notify_on_exit {
                                for c in &cs {
                                    let prev = prev_state.get(&c.id);
                                    let was_running = prev
                                        .map(|(s, _)| s.eq_ignore_ascii_case("running"))
                                        .unwrap_or(false);
                                    let now_terminal = matches!(
                                        c.status.to_ascii_lowercase().as_str(),
                                        "exited" | "stopped" | "dead"
                                    );
                                    if was_running && now_terminal {
                                        let body = match c.exit_code {
                                            Some(code) if code != 0 => format!(
                                                "{} exited with code {} ({})",
                                                c.name, code, c.image
                                            ),
                                            _ => format!("{} exited ({})", c.name, c.image),
                                        };
                                        let _ = handle
                                            .notification()
                                            .builder()
                                            .title("cgui — container exited")
                                            .body(body)
                                            .show();
                                    }
                                }
                            }
                            // Refresh prev_state to the current snapshot.
                            prev_state.clear();
                            for c in &cs {
                                prev_state.insert(c.id.clone(), (c.status.clone(), c.exit_code));
                            }

                            // Emit current poll wall-clock so the UI can show
                            // a "last updated Ns ago" indicator (A12).
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis() as u64)
                                .unwrap_or(0);
                            let _ = handle.emit("containers:tickAt", now_ms);
                            let _ = handle.emit("containers:tick", cs);
                        }
                        Err(e) => eprintln!("containers tick failed: {e:#}"),
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_containers,
            commands::list_images,
            commands::list_volumes,
            commands::list_networks,
            commands::list_stacks,
            commands::tail_logs,
            commands::inspect_container,
            commands::doctor,
            commands::scan_image,
            commands::list_updates,
            commands::pull_stream,
            commands::start_container,
            commands::stop_container,
            commands::kill_container,
            commands::delete_container,
            commands::restart_container,
            commands::start_log_stream,
            commands::start_pull,
            commands::load_prefs,
            commands::save_prefs,
            commands::delete_image,
            commands::delete_volume,
            commands::delete_network,
            commands::exec_container,
            commands::inspect_volume,
            commands::inspect_network,
            commands::inspect_image,
            commands::run_image,
            commands::stack_up,
            commands::stack_down,
            commands::stack_health,
            commands::runtime_available,
            commands::probe_runtime,
            commands::container_history,
            commands::vuln_history,
            commands::load_logs,
            commands::pty_open,
            commands::pty_write,
            commands::pty_resize,
            commands::pty_close,
            commands::import_compose,
            commands::export_compose,
            commands::snapshot_stack,
            commands::restore_stack,
            commands::restore_stack_from_path,
            hotkey_cmd::set_global_hotkey,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
