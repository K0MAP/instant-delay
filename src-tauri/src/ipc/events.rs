//! Event emission for real-time UI updates

use tauri::{AppHandle, Emitter};
use serde::{Serialize, Deserialize};
use crate::ipc::{DelayMode, Platform, StreamDeckEvent};

/// Event payload for delay changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayChangedEvent {
    pub seconds: u32,
    pub previous_seconds: u32,
}

/// Event payload for mode changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeChangedEvent {
    pub mode: DelayMode,
    pub previous_mode: DelayMode,
}

/// Event payload for buffer status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferStatusEvent {
    pub is_buffering: bool,
    pub fill_percent: f64,
    pub delay_span_sec: f64,
    pub target_delay_sec: u32,
    pub video_frames: usize,
    pub audio_frames: usize,
    pub memory_mb: f64,
}

/// Event payload for platform status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformStatusEvent {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub connected: bool,
    pub bytes_sent: u64,
    pub uptime_sec: u64,
    pub last_error: Option<String>,
}

/// Event payload for server status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatusEvent {
    pub running: bool,
    pub active_connections: usize,
    pub total_bytes_in: u64,
    pub total_bytes_out: u64,
    pub uptime_sec: u64,
}

/// Emit delay changed event
pub fn emit_delay_changed(app: &AppHandle, seconds: u32, previous: u32) {
    let _ = app.emit("delay_changed", DelayChangedEvent {
        seconds,
        previous_seconds: previous,
    });
}

/// Emit mode changed event
pub fn emit_mode_changed(app: &AppHandle, mode: DelayMode, previous: DelayMode) {
    let _ = app.emit("mode_changed", ModeChangedEvent {
        mode,
        previous_mode: previous,
    });
}

/// Emit buffer status event
pub fn emit_buffer_status(app: &AppHandle, event: BufferStatusEvent) {
    let _ = app.emit("buffer_status", event);
}

/// Emit platform status event
pub fn emit_platform_status(app: &AppHandle, event: PlatformStatusEvent) {
    let _ = app.emit("platform_status", event);
}

/// Emit server status event
pub fn emit_server_status(app: &AppHandle, event: ServerStatusEvent) {
    let _ = app.emit("server_status", event);
}

/// Emit Stream Deck event
pub fn emit_streamdeck_event(app: &AppHandle, event: StreamDeckEvent) {
    let _ = app.emit("streamdeck_event", event);
}

/// Emit overlay data event (for WebSocket clients)
pub fn emit_overlay_data(app: &AppHandle, data: crate::ipc::OverlayData) {
    let _ = app.emit("overlay_data", data);
}

/// Emit error event
pub fn emit_error(app: &AppHandle, message: String) {
    let _ = app.emit("error", serde_json::json!({ "message": message }));
}

/// Emit log event
pub fn emit_log(app: &AppHandle, level: &str, message: String) {
    let _ = app.emit("log", serde_json::json!({ "level": level, "message": message }));
}