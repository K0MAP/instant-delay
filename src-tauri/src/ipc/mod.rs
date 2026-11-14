//! IPC module - Tauri commands and shared types

pub mod commands;
pub mod events;

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Application state shared between commands
pub type AppState = crate::rtmp::SharedServer;

/// Delay modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DelayMode {
    /// Instant mode - continuous buffering, instant toggle
    Instant,
    /// Normal mode - buffer fills then streams delayed
    Normal,
    /// Disabled - pass through without delay
    Disabled,
}

impl Default for DelayMode {
    fn default() -> Self {
        DelayMode::Instant
    }
}

impl std::fmt::Display for DelayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DelayMode::Instant => write!(f, "instant"),
            DelayMode::Normal => write!(f, "normal"),
            DelayMode::Disabled => write!(f, "disabled"),
        }
    }
}

/// Delay control commands
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DelayCommand {
    SetDelay { seconds: u32 },
    GetDelay,
    ToggleDelay,
    IncreaseDelay { seconds: u32 },
    DecreaseDelay { seconds: u32 },
    SkipToLive,
    SetMode { mode: DelayMode },
}

/// Platform output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformOutput {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub url: String,
    pub stream_key: String,
    pub enabled: bool,
    pub priority: u8,
    pub backup: bool,
}

impl Default for PlatformOutput {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "New Platform".to_string(),
            platform: Platform::Custom,
            url: "rtmp://".to_string(),
            stream_key: String::new(),
            enabled: false,
            priority: 0,
            backup: false,
        }
    }
}

/// Supported platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    YouTube,
    Kick,
    TikTok,
    Restream,
    Custom,
}

impl Platform {
    pub fn default_rtmp_url(&self) -> &'static str {
        match self {
            Platform::Twitch => "rtmp://live.twitch.tv/app/",
            Platform::YouTube => "rtmp://a.rtmp.youtube.com/live2/",
            Platform::Kick => "rtmp://ingest.kick.com/app/",
            Platform::TikTok => "rtmp://push.tiktokcdn.com/live/",
            Platform::Restream => "rtmp://live.restream.io/live/",
            Platform::Custom => "rtmp://",
        }
    }

    pub fn stream_key_format(&self) -> &'static str {
        match self {
            Platform::Twitch => "live_XXXXXXXX_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
            Platform::YouTube => "XXXX-XXXX-XXXX-XXXX-XXXX",
            Platform::Kick => "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
            Platform::TikTok => "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
            Platform::Restream => "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
            Platform::Custom => "",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Twitch => write!(f, "Twitch"),
            Platform::YouTube => write!(f, "YouTube"),
            Platform::Kick => write!(f, "Kick"),
            Platform::TikTok => write!(f, "TikTok"),
            Platform::Restream => write!(f, "Restream"),
            Platform::Custom => write!(f, "Custom RTMP"),
        }
    }
}

/// Hotkey configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub toggle_delay: Option<String>,
    pub increase_delay: Option<String>,
    pub decrease_delay: Option<String>,
    pub skip_to_live: Option<String>,
    pub toggle_video: Option<String>,
    pub toggle_audio: Option<String>,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            toggle_delay: Some("Control+Shift+D".to_string()),
            increase_delay: Some("Control+Shift+Up".to_string()),
            decrease_delay: Some("Control+Shift+Down".to_string()),
            skip_to_live: Some("Control+Shift+L".to_string()),
            toggle_video: Some("Control+Shift+V".to_string()),
            toggle_audio: Some("Control+Shift+A".to_string()),
        }
    }
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    // Server
    pub bind_address: String,
    pub port: u16,
    pub auto_start_server: bool,

    // Delay
    pub default_delay_sec: u32,
    pub max_delay_sec: u32,
    pub delay_mode: DelayMode,
    pub delay_step_sec: u32,

    // Buffer
    pub buffer_max_memory_mb: usize,
    pub buffer_keyframe_only: bool,

    // Platforms
    pub platform_outputs: Vec<PlatformOutput>,

    // Overlay
    pub overlay_enabled: bool,
    pub overlay_port: u16,
    pub overlay_theme: OverlayTheme,

    // Hotkeys
    pub hotkeys: HotkeyConfig,
    pub global_hotkeys_enabled: bool,

    // Stream Deck
    pub streamdeck_enabled: bool,
    pub streamdeck_port: u16,

    // Web UI
    pub web_ui_enabled: bool,
    pub web_ui_port: u16,
    pub web_ui_auth_token: Option<String>,

    // Advanced
    pub log_level: String,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 1935,
            auto_start_server: true,
            default_delay_sec: 10,
            max_delay_sec: 600,
            delay_mode: DelayMode::Instant,
            delay_step_sec: 5,
            buffer_max_memory_mb: 512,
            buffer_keyframe_only: false,
            platform_outputs: Vec::new(),
            overlay_enabled: true,
            overlay_port: 8080,
            overlay_theme: OverlayTheme::default(),
            hotkeys: HotkeyConfig::default(),
            global_hotkeys_enabled: true,
            streamdeck_enabled: false,
            streamdeck_port: 3000,
            web_ui_enabled: true,
            web_ui_port: 8081,
            web_ui_auth_token: None,
            log_level: "info".to_string(),
            minimize_to_tray: true,
            start_minimized: false,
            check_updates: true,
        }
    }
}

/// Overlay theme
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayTheme {
    pub background_color: String,
    pub text_color: String,
    pub accent_color: String,
    pub font_family: String,
    pub font_size: u16,
    pub border_radius: u8,
    pub padding: u8,
    pub show_background: bool,
    pub animation_enabled: bool,
}

impl Default for OverlayTheme {
    fn default() -> Self {
        Self {
            background_color: "rgba(0, 0, 0, 0.8)".to_string(),
            text_color: "#ffffff".to_string(),
            accent_color: "#ff6b35".to_string(),
            font_family: "system-ui, -apple-system, sans-serif".to_string(),
            font_size: 14,
            border_radius: 8,
            padding: 12,
            show_background: true,
            animation_enabled: true,
        }
    }
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub cpu_cores: usize,
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub obs_running: bool,
    pub obs_version: Option<String>,
}

/// Server status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatusResponse {
    pub running: bool,
    pub bind_address: String,
    pub port: u16,
    pub active_connections: usize,
    pub total_bytes_in: u64,
    pub total_bytes_out: u64,
    pub uptime_sec: u64,
    pub current_delay_sec: u32,
    pub delay_mode: DelayMode,
    pub is_buffering: bool,
    pub buffer_fill_percent: f64,
    pub platform_outputs: Vec<PlatformOutputStatus>,
}

/// Platform output status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformOutputStatus {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub connected: bool,
    pub bytes_sent: u64,
    pub uptime_sec: u64,
    pub last_error: Option<String>,
}

/// Overlay data for WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayData {
    pub delay_sec: u32,
    pub delay_mode: DelayMode,
    pub is_buffering: bool,
    pub buffer_fill_percent: f64,
    pub status_text: String,
    pub status_color: String,
    pub platform_count: usize,
    pub connected_platforms: usize,
}

/// Stream Deck events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum StreamDeckEvent {
    DelayChanged { seconds: u32 },
    ModeChanged { mode: DelayMode },
    BufferingChanged { buffering: bool },
    PlatformStatusChanged { id: String, connected: bool },
    Error { message: String },
}