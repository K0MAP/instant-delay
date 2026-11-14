//! InstantDelay - Stream delay control for OBS
//! Change delay while live without restarting OBS

mod rtmp;
mod ipc;
mod config;
mod tray;
mod hotkeys;
mod web;
mod streamdeck;
mod overlay;

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, error, warn};
use tauri::Manager;

use crate::ipc::AppState;
use crate::config::SettingsManager;
use crate::tray::TrayManager;
use crate::hotkeys::HotkeyManager;
use crate::web::WebServer;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "instant_delay=info,tauri=info".into()),
        )
        .init();

    info!("Starting InstantDelay v{}", env!("CARGO_PKG_VERSION"));
    let settings = tauri::async_runtime::block_on(SettingsManager::load()).unwrap_or_default();
    let shared_state: AppState = Arc::new(RwLock::new(None));

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(shared_state.clone())
        .invoke_handler(tauri::generate_handler![
            ipc::commands::start_rtmp_server,
            ipc::commands::stop_rtmp_server,
            ipc::commands::get_rtmp_status,
            ipc::commands::set_delay,
            ipc::commands::get_delay,
            ipc::commands::toggle_delay,
            ipc::commands::increase_delay,
            ipc::commands::decrease_delay,
            ipc::commands::skip_to_live,
            ipc::commands::add_platform_output,
            ipc::commands::remove_platform_output,
            ipc::commands::update_platform_output,
            ipc::commands::get_platform_outputs,
            ipc::commands::test_platform_connection,
            ipc::commands::get_settings,
            ipc::commands::update_settings,
            ipc::commands::reset_settings,
            ipc::commands::get_overlay_url,
            ipc::commands::regenerate_overlay,
            ipc::commands::get_streamdeck_status,
            ipc::commands::send_streamdeck_update,
            hotkeys::commands::register_hotkeys,
            hotkeys::commands::unregister_hotkeys,
            hotkeys::commands::get_hotkey_config,
            hotkeys::commands::update_hotkey_config,
            ipc::commands::get_system_info,
            ipc::commands::check_obs_running,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            app.manage(TrayManager::new(app_handle.clone(), shared_state.clone())?);
            app.manage(HotkeyManager::new(app_handle.clone(), shared_state.clone()));
            let web_server = Arc::new(Mutex::new(WebServer::new(
                shared_state.clone(), &settings,
            )));
            app.manage(web_server.clone());

            tauri::async_runtime::spawn(async move {
                if settings.auto_start_server {
                    if let Err(e) = ipc::commands::start_rtmp_server(app_handle.state::<AppState>()).await {
                        error!("Failed to auto-start RTMP server: {}", e);
                    }
                }
                if let Err(e) = hotkeys::commands::register_hotkeys(app_handle.clone(), app_handle.state::<HotkeyManager>()).await {
                    warn!("Failed to register hotkeys: {}", e);
                }
                if let Err(e) = web_server.lock().await.start().await {
                    error!("Failed to start web server: {}", e);
                }
            });

            info!("InstantDelay initialized successfully");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("Error running Tauri application");
}
