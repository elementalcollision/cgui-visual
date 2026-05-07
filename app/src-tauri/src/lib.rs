mod commands;
pub mod compose;
mod doctor;
mod fixtures;
pub mod model;
mod prefs;
pub mod runtime;
pub mod stacks;
pub mod state;
pub mod trivy;
pub mod updates;

use std::sync::Arc;
use std::time::Duration;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(state::History::new()))
        .setup(|app| {
            // Seed the runtime binary selection from persisted prefs.
            runtime::set_bin(&prefs::Prefs::load().runtime);

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
                            let running = cs
                                .iter()
                                .filter(|c| c.status.eq_ignore_ascii_case("running"))
                                .count();
                            if let Some(t) = handle.tray_by_id("main") {
                                let _ = t.set_title(Some(fmt_tray_title(Some(running))));
                            }
                            let _ = count_item_for_tick
                                .set_text(format!("Containers: {running} running"));
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
            commands::import_compose,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
