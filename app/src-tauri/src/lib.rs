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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem, SubmenuBuilder},
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

// ─── Menubar mode / window visibility ───────────────────────────────────

// Mirror of `Prefs::menubar_mode`, kept in memory so the window-close
// handler and tray actions never touch disk. Seeded in setup and updated by
// `set_menubar_mode` whenever the frontend saves prefs, so toggling the
// setting takes effect immediately without a restart.
static MENUBAR_MODE: AtomicBool = AtomicBool::new(false);

// Set when the window is shown so the polling loop refreshes right away
// instead of waiting out the slow hidden-window interval.
static WAKE_TICK: AtomicBool = AtomicBool::new(false);

// Polling cadence: fast while the window is on screen, slow while hidden in
// the tray. Polling never stops entirely: the tray count and exit
// notifications keep working with the window closed.
const TICK_VISIBLE: Duration = Duration::from_secs(2);
const TICK_HIDDEN: Duration = Duration::from_secs(10);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum CloseAction {
    // Let the close proceed. With the only window gone Tauri exits the app
    // (the long-standing behaviour when menubar mode is off).
    Quit,
    // Cancel the close and hide the window; the app lives on in the tray.
    HideToTray,
}

fn close_action(menubar_mode: bool) -> CloseAction {
    if menubar_mode {
        CloseAction::HideToTray
    } else {
        CloseAction::Quit
    }
}

fn tick_interval(window_visible: bool) -> Duration {
    if window_visible {
        TICK_VISIBLE
    } else {
        TICK_HIDDEN
    }
}

// The Dock icon is only dropped (Accessory policy) when menubar mode is on
// and the window is hidden. Otherwise cgui is an ordinary Dock app.
fn wants_dock_icon(menubar_mode: bool, window_visible: bool) -> bool {
    !menubar_mode || window_visible
}

#[cfg(target_os = "macos")]
fn apply_dock_policy<R: tauri::Runtime>(app: &tauri::AppHandle<R>, window_visible: bool) {
    let policy = if wants_dock_icon(MENUBAR_MODE.load(Ordering::Relaxed), window_visible) {
        tauri::ActivationPolicy::Regular
    } else {
        tauri::ActivationPolicy::Accessory
    };
    let _ = app.set_activation_policy(policy);
}

#[cfg(not(target_os = "macos"))]
fn apply_dock_policy<R: tauri::Runtime>(_app: &tauri::AppHandle<R>, _window_visible: bool) {}

fn main_window_visible<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    app.get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    // Restore the Dock icon before showing so the window comes up as a
    // normal, focusable app window.
    apply_dock_policy(app, true);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    WAKE_TICK.store(true, Ordering::Relaxed);
}

fn hide_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    apply_dock_policy(app, false);
}

// Called from setup and from `save_prefs` so the pref applies live.
pub(crate) fn set_menubar_mode<R: tauri::Runtime>(app: &tauri::AppHandle<R>, on: bool) {
    MENUBAR_MODE.store(on, Ordering::Relaxed);
    apply_dock_policy(app, main_window_visible(app));
}

// Runs once from RunEvent::Exit (tray Quit, Cmd-Q, last window closed).
// Children outlive us otherwise: the PTY children and `container logs -f`.
fn on_exit() {
    pty::close_all();
    // Kill every `container logs -f` child, then let the batching writer
    // put already-queued lines on disk before the process goes away.
    commands::stop_all_log_streams();
    history::flush_logs();
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
            show_main_window(&app_handle);
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
        // Menubar mode: with it on, closing the main window hides it and
        // leaves the app running in the tray. With it off the close
        // proceeds and, as the only window, ends the app (Tauri default).
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main"
                    && close_action(MENUBAR_MODE.load(Ordering::Relaxed)) == CloseAction::HideToTray
                {
                    api.prevent_close();
                    hide_main_window(window.app_handle());
                }
            }
        })
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
            MENUBAR_MODE.store(initial_prefs.menubar_mode, Ordering::Relaxed);

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

            // macOS app menu. Tauri's default menu is fine but doesn't
            // include the two affordances users expect from the
            // application menu — Check for Updates and Settings — so we
            // build a custom one. Menu items emit Tauri events that the
            // frontend listens for in App.tsx; that keeps menu wiring
            // in Rust and behaviour wiring in TS without a backend
            // round-trip per click.
            //
            // The Edit submenu is critical for any text-input UX (cut /
            // copy / paste in form fields and the embedded terminal),
            // so we always include it. Window submenu rounds out the
            // standard macOS conventions users expect from a Cocoa app.
            let app_submenu = SubmenuBuilder::new(app, "cgui")
                .about(None)
                .separator()
                .item(&MenuItem::with_id(
                    app,
                    "menu:check-updates",
                    "Check for Updates…",
                    true,
                    None::<&str>,
                )?)
                .item(&MenuItem::with_id(
                    app,
                    "menu:settings",
                    "Settings…",
                    true,
                    Some("CmdOrCtrl+,"),
                )?)
                .separator()
                .services()
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;
            let edit_submenu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;
            let window_submenu = SubmenuBuilder::new(app, "Window")
                .minimize()
                .maximize()
                .separator()
                .close_window()
                .build()?;
            let app_menu = Menu::with_items(app, &[&app_submenu, &edit_submenu, &window_submenu])?;
            app.set_menu(app_menu)?;
            // Single global handler for both custom items. Each just
            // emits an event; the frontend owns the actual flow so we
            // don't have to plumb modal state through the backend.
            app.on_menu_event(|app_handle, event| match event.id.as_ref() {
                "menu:check-updates" => {
                    let _ = app_handle.emit("menu:check-updates", ());
                }
                "menu:settings" => {
                    let _ = app_handle.emit("menu:settings", ());
                }
                _ => {}
            });

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
                    "show" => show_main_window(app),
                    "hide" => hide_main_window(app),
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
                                hide_main_window(app);
                            } else {
                                show_main_window(app);
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
                // Wake every second to check whether a poll is due: the
                // interval depends on window visibility, and showing the
                // window requests an immediate refresh (WAKE_TICK).
                let mut last = Instant::now() - TICK_HIDDEN;
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let due = last.elapsed() >= tick_interval(main_window_visible(&handle));
                    if !(WAKE_TICK.swap(false, Ordering::Relaxed) || due) {
                        continue;
                    }
                    last = Instant::now();
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
                        Err(e) => {
                            eprintln!("containers tick failed: {e:#}");
                            // Let the UI show the failure state instead of
                            // freezing on the last good list. Plain string,
                            // re-sent each tick; the frontend dedupes.
                            let _ = handle.emit("containers:tickError", format!("{e:#}"));
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_containers,
            commands::list_images,
            commands::list_images_checked,
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
            commands::clean_containers,
            commands::start_log_stream,
            commands::stop_log_stream,
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
            commands::tag_image,
            commands::prune_containers,
            commands::prune_images,
            commands::prune_volumes,
            commands::prune_networks,
            commands::system_df,
            commands::save_image,
            commands::load_image,
            commands::start_push,
            commands::copy_path,
            commands::export_container,
            commands::registry_list,
            commands::registry_login,
            commands::registry_logout,
            commands::builder_status,
            commands::builder_start,
            commands::builder_stop,
            commands::builder_delete,
            commands::system_start,
            commands::system_stop,
            commands::system_restart,
            commands::system_status,
            commands::start_build,
            commands::machine_list,
            commands::machine_create,
            commands::machine_stop,
            commands::machine_delete,
            commands::machine_set_default,
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::Exit => on_exit(),
            // Dock icon clicked while no window is visible (not reachable
            // in menubar mode, where the Dock icon is hidden, but cheap).
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => show_main_window(app),
            _ => {
                let _ = app;
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_action_follows_menubar_mode() {
        assert_eq!(close_action(true), CloseAction::HideToTray);
        assert_eq!(close_action(false), CloseAction::Quit);
    }

    #[test]
    fn tick_slows_while_hidden_but_never_stops() {
        assert_eq!(tick_interval(true), Duration::from_secs(2));
        assert_eq!(tick_interval(false), Duration::from_secs(10));
        assert!(tick_interval(false) > tick_interval(true));
    }

    #[test]
    fn dock_icon_only_hidden_in_menubar_mode_with_window_hidden() {
        assert!(wants_dock_icon(false, true));
        assert!(wants_dock_icon(false, false));
        assert!(wants_dock_icon(true, true));
        assert!(!wants_dock_icon(true, false));
    }
}
