//! Global hotkey management using Tauri's global shortcut plugin

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{AppHandle, Manager, Emitter};
use crate::ipc::{AppState, HotkeyConfig};
use tracing::{info, warn, debug, error};
use serde::Serialize;

/// Hotkey manager using Tauri's global shortcut plugin
pub struct HotkeyManager {
    app_handle: AppHandle,
    state: AppState,
    registered_shortcuts: Arc<RwLock<HashMap<String, HotkeyAction>>>,
}

/// Hotkey actions
#[derive(Debug, Clone)]
pub enum HotkeyAction {
    ToggleDelay,
    IncreaseDelay,
    DecreaseDelay,
    SkipToLive,
    ToggleVideo,
    ToggleAudio,
}

impl HotkeyManager {
    /// Create new hotkey manager
    pub fn new(app_handle: AppHandle, state: AppState) -> Self {
        Self {
            app_handle,
            state,
            registered_shortcuts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register all hotkeys from config - stores them for execution, frontend handles actual registration
    pub async fn register_all(&self, config: &HotkeyConfig) -> anyhow::Result<()> {
        // Clear existing
        self.unregister_all().await?;

        let shortcuts = self.registered_shortcuts.clone();

        // Register each hotkey mapping
        if let Some(hk) = &config.toggle_delay {
            Self::store_shortcut(shortcuts.clone(), hk, HotkeyAction::ToggleDelay).await;
        }
        if let Some(hk) = &config.increase_delay {
            Self::store_shortcut(shortcuts.clone(), hk, HotkeyAction::IncreaseDelay).await;
        }
        if let Some(hk) = &config.decrease_delay {
            Self::store_shortcut(shortcuts.clone(), hk, HotkeyAction::DecreaseDelay).await;
        }
        if let Some(hk) = &config.skip_to_live {
            Self::store_shortcut(shortcuts.clone(), hk, HotkeyAction::SkipToLive).await;
        }
        if let Some(hk) = &config.toggle_video {
            Self::store_shortcut(shortcuts.clone(), hk, HotkeyAction::ToggleVideo).await;
        }
        if let Some(hk) = &config.toggle_audio {
            Self::store_shortcut(shortcuts.clone(), hk, HotkeyAction::ToggleAudio).await;
        }

        info!("Global hotkeys stored for execution via Tauri plugin");
        Ok(())
    }

    /// Store a shortcut mapping for later execution
    async fn store_shortcut(
        shortcuts: Arc<RwLock<HashMap<String, HotkeyAction>>>,
        hotkey_str: &str,
        action: HotkeyAction,
    ) {
        let accelerator = Self::parse_hotkey(hotkey_str).unwrap_or_else(|_| hotkey_str.to_string());
        let handler_name = format!("hotkey_{}", accelerator.replace('+', "_").to_lowercase());
        shortcuts.write().await.insert(handler_name, action.clone());
        debug!("Stored hotkey mapping: {} -> {:?}", accelerator, action);
    }

    /// Unregister all hotkeys
    pub async fn unregister_all(&self) -> anyhow::Result<()> {
        self.registered_shortcuts.write().await.clear();
        info!("All hotkey mappings cleared");
        Ok(())
    }

    /// Execute hotkey action by handler name (called from frontend via Tauri command)
    pub async fn execute_action(&self, handler_name: &str) {
        let action = {
            let shortcuts = self.registered_shortcuts.read().await;
            shortcuts.get(handler_name).cloned()
        };

        if let Some(action) = action {
            Self::execute_action_static(&self.app_handle, &self.state, action).await;
        }
    }

    /// Execute hotkey action (static method)
    async fn execute_action_static(
        app: &AppHandle,
        state: &AppState,
        action: HotkeyAction,
    ) {
        let guard = state.read().await;
        if let Some(server) = guard.as_ref() {
            match action {
                HotkeyAction::ToggleDelay => {
                    let _ = server.toggle_delay().await;
                    let _ = app.emit("hotkey_toggle_delay", ());
                }
                HotkeyAction::IncreaseDelay => {
                    let current = server.buffer().get_delay().await;
                    let new = (current + 5).min(600);
                    let _ = server.set_delay(new).await;
                    let _ = app.emit("hotkey_increase_delay", serde_json::json!({ "seconds": new }));
                }
                HotkeyAction::DecreaseDelay => {
                    let current = server.buffer().get_delay().await;
                    let new = current.saturating_sub(5).max(1);
                    let _ = server.set_delay(new).await;
                    let _ = app.emit("hotkey_decrease_delay", serde_json::json!({ "seconds": new }));
                }
                HotkeyAction::SkipToLive => {
                    let _ = server.skip_to_live().await;
                    let _ = app.emit("hotkey_skip_to_live", ());
                }
                HotkeyAction::ToggleVideo => {
                    let _ = app.emit("hotkey_toggle_video", ());
                }
                HotkeyAction::ToggleAudio => {
                    let _ = app.emit("hotkey_toggle_audio", ());
                }
            }
        }
    }

    /// Parse hotkey string (e.g., "Control+Shift+D") into Tauri accelerator format
    fn parse_hotkey(hotkey_str: &str) -> anyhow::Result<String> {
        let parts: Vec<&str> = hotkey_str.split('+').collect();
        let mut modifiers = Vec::new();
        let mut main_key = None;

        for part in parts {
            let part_lower = part.trim().to_lowercase();
            match part_lower.as_str() {
                "control" | "ctrl" => modifiers.push("Control"),
                "shift" => modifiers.push("Shift"),
                "alt" | "option" => modifiers.push("Alt"),
                "meta" | "command" | "cmd" | "super" => modifiers.push("Meta"),
                _ => {
                    main_key = Some(Self::key_to_accelerator(&part_lower));
                }
            }
        }

        let mut accelerator = modifiers.join("+");
        if let Some(key) = main_key {
            if !accelerator.is_empty() {
                accelerator.push('+');
            }
            accelerator.push_str(&key);
        }

        if accelerator.is_empty() {
            anyhow::bail!("Invalid hotkey: {}", hotkey_str);
        }

        Ok(accelerator)
    }

    /// Convert key name to Tauri accelerator format
    fn key_to_accelerator(key: &str) -> String {
        match key {
            "up" => "Up".to_string(),
            "down" => "Down".to_string(),
            "left" => "Left".to_string(),
            "right" => "Right".to_string(),
            "space" => "Space".to_string(),
            "enter" => "Enter".to_string(),
            "escape" | "esc" => "Escape".to_string(),
            "tab" => "Tab".to_string(),
            "backspace" => "Backspace".to_string(),
            "delete" => "Delete".to_string(),
            "home" => "Home".to_string(),
            "end" => "End".to_string(),
            "pageup" => "PageUp".to_string(),
            "pagedown" => "PageDown".to_string(),
            "insert" => "Insert".to_string(),
            "f1" => "F1".to_string(),
            "f2" => "F2".to_string(),
            "f3" => "F3".to_string(),
            "f4" => "F4".to_string(),
            "f5" => "F5".to_string(),
            "f6" => "F6".to_string(),
            "f7" => "F7".to_string(),
            "f8" => "F8".to_string(),
            "f9" => "F9".to_string(),
            "f10" => "F10".to_string(),
            "f11" => "F11".to_string(),
            "f12" => "F12".to_string(),
            "0" => "0".to_string(),
            "1" => "1".to_string(),
            "2" => "2".to_string(),
            "3" => "3".to_string(),
            "4" => "4".to_string(),
            "5" => "5".to_string(),
            "6" => "6".to_string(),
            "7" => "7".to_string(),
            "8" => "8".to_string(),
            "9" => "9".to_string(),
            "a" => "A".to_string(),
            "b" => "B".to_string(),
            "c" => "C".to_string(),
            "d" => "D".to_string(),
            "e" => "E".to_string(),
            "f" => "F".to_string(),
            "g" => "G".to_string(),
            "h" => "H".to_string(),
            "i" => "I".to_string(),
            "j" => "J".to_string(),
            "k" => "K".to_string(),
            "l" => "L".to_string(),
            "m" => "M".to_string(),
            "n" => "N".to_string(),
            "o" => "O".to_string(),
            "p" => "P".to_string(),
            "q" => "Q".to_string(),
            "r" => "R".to_string(),
            "s" => "S".to_string(),
            "t" => "T".to_string(),
            "u" => "U".to_string(),
            "v" => "V".to_string(),
            "w" => "W".to_string(),
            "x" => "X".to_string(),
            "y" => "Y".to_string(),
            "z" => "Z".to_string(),
            _ => key.to_uppercase(),
        }
    }
}

// Tauri command handlers for hotkeys
pub mod commands {
    use super::*;
    use tauri::{State, AppHandle};

    /// Register global hotkeys (stores mappings, frontend handles actual registration)
    #[tauri::command]
    pub async fn register_hotkeys(
        app: AppHandle,
        manager: State<'_, HotkeyManager>,
    ) -> Result<(), String> {
        let settings = crate::config::SettingsManager::load().await.unwrap_or_default();

        if !settings.global_hotkeys_enabled {
            return Ok(());
        }

        manager.register_all(&settings.hotkeys).await.map_err(|e| e.to_string())
    }

    /// Unregister global hotkeys
    #[tauri::command]
    pub async fn unregister_hotkeys(manager: State<'_, HotkeyManager>) -> Result<(), String> {
        manager.unregister_all().await.map_err(|e| e.to_string())
    }

    /// Get hotkey configuration
    #[tauri::command]
    pub async fn get_hotkey_config() -> Result<HotkeyConfig, String> {
        let settings = crate::config::SettingsManager::load().await.unwrap_or_default();
        Ok(settings.hotkeys)
    }

    /// Update hotkey configuration
    #[tauri::command]
    pub async fn update_hotkey_config(
        app: AppHandle,
        manager: State<'_, HotkeyManager>,
        config: HotkeyConfig,
    ) -> Result<(), String> {
        let mut settings = crate::config::SettingsManager::load().await.unwrap_or_default();
        settings.hotkeys = config.clone();
        crate::config::SettingsManager::save(&settings).await.map_err(|e| e.to_string())?;

        // Re-register with new config
        if settings.global_hotkeys_enabled {
            manager.register_all(&config).await.map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Execute hotkey action (called from frontend when global shortcut is triggered)
    #[tauri::command]
    pub async fn execute_hotkey(
        app: AppHandle,
        manager: State<'_, HotkeyManager>,
        handler: String,
    ) -> Result<(), String> {
        manager.execute_action(&handler).await;
        Ok(())
    }
}