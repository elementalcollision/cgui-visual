mod commands;
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
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(Arc::new(state::History::new()))
        .setup(|app| {
            // Seed the runtime binary selection from persisted prefs.
            runtime::set_bin(&prefs::Prefs::load().runtime);

            let handle = app.handle().clone();
            let history: Arc<state::History> = app.state::<Arc<state::History>>().inner().clone();
            tauri::async_runtime::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(2));
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    tick.tick().await;
                    if !runtime::available().await {
                        continue;
                    }
                    match state::poll_once(&history).await {
                        Ok(cs) => {
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
