//! System tray management

use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{AppHandle, Manager, Runtime, tray::TrayIconBuilder, menu::{MenuBuilder, MenuItemBuilder}, Emitter};
use crate::rtmp::SharedServer;
use crate::ipc::AppState;
use tracing::{info, debug};

/// Tray manager
pub struct TrayManager {
    app_handle: AppHandle,
    state: AppState,
}

impl TrayManager {
    /// Create new tray manager
    pub fn new(app_handle: AppHandle, state: AppState) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = Self {
            app_handle: app_handle.clone(),
            state,
        };

        manager.create_tray()?;
        Ok(manager)
    }

    /// Create system tray icon and menu
    fn create_tray(&self) -> Result<(), Box<dyn std::error::Error>> {
        let app = &self.app_handle;

        // Build menu
        let show_item = MenuItemBuilder::new("Show Window")
            .id("show")
            .accelerator("CommandOrControl+Shift+I")
            .build(app)?;

        let toggle_delay_item = MenuItemBuilder::new("Toggle Delay")
            .id("toggle_delay")
            .accelerator("CommandOrControl+Shift+D")
            .build(app)?;

        let increase_item = MenuItemBuilder::new("Increase Delay (+5s)")
            .id("increase_delay")
            .accelerator("CommandOrControl+Shift+Up")
            .build(app)?;

        let decrease_item = MenuItemBuilder::new("Decrease Delay (-5s)")
            .id("decrease_delay")
            .accelerator("CommandOrControl+Shift+Down")
            .build(app)?;

        let skip_live_item = MenuItemBuilder::new("Skip to Live")
            .id("skip_to_live")
            .accelerator("CommandOrControl+Shift+L")
            .build(app)?;

        let overlay_item = MenuItemBuilder::new("Open Overlay URL")
            .id("open_overlay")
            .build(app)?;

        let copy_key_item = MenuItemBuilder::new("Copy Stream Key")
            .id("copy_stream_key")
            .build(app)?;

        let settings_item = MenuItemBuilder::new("Settings")
            .id("settings")
            .accelerator("CommandOrControl+,")
            .build(app)?;

        let quit_item = MenuItemBuilder::new("Quit")
            .id("quit")
            .accelerator("CommandOrControl+Q")
            .build(app)?;

        let menu = MenuBuilder::new(app)
            .items(&[&show_item])
            .separator()
            .items(&[&toggle_delay_item, &increase_item, &decrease_item, &skip_live_item])
            .separator()
            .items(&[&overlay_item, &copy_key_item])
            .separator()
            .items(&[&settings_item, &quit_item])
            .build()?;

        // Create tray icon
        let _tray = TrayIconBuilder::new()
            .icon(app.default_window_icon().cloned().unwrap())
            .menu(&menu)
            .on_menu_event(|app, event| {
                Self::handle_menu_event(app, event.id.as_ref());
            })
            .on_tray_icon_event(|tray, event| {
                if let tauri::tray::TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                    if let Some(window) = tray.app_handle().get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            })
            .build(app)?;

        Ok(())
    }

    /// Handle menu events
    fn handle_menu_event(app: &AppHandle, id: &str) {
        match id {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "toggle_delay" => {
                let _ = app.emit("tray_toggle_delay", ());
            }
            "increase_delay" => {
                let _ = app.emit("tray_increase_delay", ());
            }
            "decrease_delay" => {
                let _ = app.emit("tray_decrease_delay", ());
            }
            "skip_to_live" => {
                let _ = app.emit("tray_skip_to_live", ());
            }
            "open_overlay" => {
                let _ = app.emit("tray_open_overlay", ());
            }
            "copy_stream_key" => {
                let _ = app.emit("tray_copy_stream_key", ());
            }
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = app.emit("navigate_settings", ());
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        }
    }

    /// Update tray tooltip with current status
    pub async fn update_tooltip(&self, delay_sec: u32, buffering: bool, connected: usize) {
        let status = if buffering { "Buffering" } else { "Live" };
        let tooltip = format!("InstantDelay - {}s delay - {} - {} outputs", delay_sec, status, connected);

        // Note: Tauri 2 doesn't have direct tooltip update, would need to recreate
        debug!("Tray tooltip: {}", tooltip);
    }

    /// Update tray menu item states
    pub async fn update_menu_state(&self, delay_enabled: bool, delay_sec: u32) {
        // Would update menu item text/enabled state
        debug!("Menu state: enabled={}, delay={}s", delay_enabled, delay_sec);
    }
}