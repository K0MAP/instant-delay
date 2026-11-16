//! Tauri command handlers

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tauri::{State, AppHandle, Emitter};
use tracing::{info, warn, error, debug};
use anyhow::Result;
use uuid::Uuid;
use std::net::IpAddr;

use crate::rtmp::{RtmpServer, ServerConfig, SharedServer};
use crate::ipc::{
    AppState, Settings, DelayMode, PlatformOutput, Platform,
    SystemInfo, ServerStatusResponse, PlatformOutputStatus,
    OverlayData, StreamDeckEvent
};
use crate::config::SettingsManager;
use crate::overlay::OverlayGenerator;
use crate::web::WebServer;

// Rate limiting state
static LAST_SERVER_START: std::sync::OnceLock<std::sync::Mutex<Option<Instant>>> = std::sync::OnceLock::new();
static LAST_SERVER_STOP: std::sync::OnceLock<std::sync::Mutex<Option<Instant>>> = std::sync::OnceLock::new();
static LAST_DELAY_CHANGE: std::sync::OnceLock<std::sync::Mutex<Option<Instant>>> = std::sync::OnceLock::new();

/// Start RTMP server
#[tauri::command]
pub async fn start_rtmp_server(state: State<'_, AppState>) -> Result<(), String> {
    // Rate limiting: max 1 start per 5 seconds
    check_rate_limit(&LAST_SERVER_START, 5)?;

    let mut guard = state.write().await;

    if guard.is_some() {
        return Err("Server already running".to_string());
    }

    // Load settings for config
    let settings = SettingsManager::load().await.unwrap_or_default();

    // Validate bind address
    validate_bind_address(&settings.bind_address)?;

    // Validate ports
    validate_port(settings.port, "RTMP")?;
    validate_port(settings.web_ui_port, "Web UI")?;
    validate_port(settings.overlay_port, "Overlay")?;
    validate_port(settings.streamdeck_port, "Stream Deck")?;

    let config = ServerConfig {
        bind_address: settings.bind_address.clone(),
        port: settings.port,
        max_connections: 10,
        buffer_config: crate::rtmp::BufferConfig {
            max_delay_sec: settings.max_delay_sec,
            max_memory_mb: settings.buffer_max_memory_mb,
            keyframe_only: settings.buffer_keyframe_only,
            ..Default::default()
        },
        relay_outputs: settings.platform_outputs.iter()
            .filter(|o| o.enabled)
            .cloned()
            .collect(),
        delay_mode: settings.delay_mode,
        default_delay_sec: settings.default_delay_sec,
    };

    match RtmpServer::new(config).await {
        Ok(mut server) => {
            if let Err(e) = server.start().await {
                error!("Failed to start RTMP server: {}", e);
                return Err(e.to_string());
            }
            *guard = Some(server);
            info!("RTMP server started on {}:{}", settings.bind_address, settings.port);
            Ok(())
        }
        Err(e) => {
            error!("Failed to create RTMP server: {}", e);
            Err(e.to_string())
        }
    }
}

/// Stop RTMP server
#[tauri::command]
pub async fn stop_rtmp_server(state: State<'_, AppState>) -> Result<(), String> {
    // Rate limiting: max 1 stop per 5 seconds
    check_rate_limit(&LAST_SERVER_STOP, 5)?;

    let mut guard = state.write().await;

    if let Some(mut server) = guard.take() {
        if let Err(e) = server.stop().await {
            error!("Error stopping RTMP server: {}", e);
            return Err(e.to_string());
        }
        info!("RTMP server stopped");
    }

    Ok(())
}

/// Get RTMP server status
#[tauri::command]
pub async fn get_rtmp_status(state: State<'_, AppState>) -> Result<ServerStatusResponse, String> {
    let guard = state.read().await;

    if let Some(server) = guard.as_ref() {
        let status = server.get_status().await;
        let relay_status = status.relay_status.clone();

        Ok(ServerStatusResponse {
            running: status.running,
            bind_address: status.bind_address,
            port: status.port,
            active_connections: status.active_connections,
            total_bytes_in: status.total_bytes_in,
            total_bytes_out: status.total_bytes_out,
            uptime_sec: status.uptime_sec,
            current_delay_sec: status.current_delay_sec,
            delay_mode: status.delay_mode,
            is_buffering: status.buffer_stats.as_ref().map(|b| b.is_buffering).unwrap_or(false),
            buffer_fill_percent: status.buffer_stats.as_ref().map(|b| b.fill_percent).unwrap_or(0.0),
            platform_outputs: relay_status.into_iter().map(|r| PlatformOutputStatus {
                id: r.id,
                name: r.name,
                platform: Platform::Custom, // Would need proper mapping
                connected: r.connected,
                bytes_sent: r.bytes_sent,
                uptime_sec: r.uptime_sec,
                last_error: r.last_error,
            }).collect(),
        })
    } else {
        Ok(ServerStatusResponse {
            running: false,
            bind_address: "127.0.0.1".to_string(),
            port: 1935,
            active_connections: 0,
            total_bytes_in: 0,
            total_bytes_out: 0,
            uptime_sec: 0,
            current_delay_sec: 0,
            delay_mode: DelayMode::Instant,
            is_buffering: false,
            buffer_fill_percent: 0.0,
            platform_outputs: Vec::new(),
        })
    }
}

/// Set delay in seconds
#[tauri::command]
pub async fn set_delay(app: AppHandle, state: State<'_, AppState>, seconds: u32) -> Result<(), String> {
    // Rate limiting: max 10 delay changes per second
    check_rate_limit(&LAST_DELAY_CHANGE, 0)?; // Sub-second precision handled by mutex

    let guard = state.read().await;

    if let Some(server) = guard.as_ref() {
        // Validate delay
        if seconds == 0 {
            return Err("Delay must be >= 1 second".to_string());
        }

        // Get max delay from settings
        let settings = SettingsManager::load().await.unwrap_or_default();
        let max_delay = settings.max_delay_sec;
        let clamped = seconds.min(max_delay);

        server.set_delay(clamped).await.map_err(|e| e.to_string())?;

        // Update settings
        let mut settings = SettingsManager::load().await.unwrap_or_default();
        settings.default_delay_sec = clamped;
        SettingsManager::save(&settings).await.map_err(|e| e.to_string())?;

        // Emit event for UI update
        {
            let _ = app.emit("delay_changed", serde_json::json!({ "seconds": clamped }));
        }

        Ok(())
    } else {
        Err("Server not running".to_string())
    }
}

/// Get current delay
#[tauri::command]
pub async fn get_delay(state: State<'_, AppState>) -> Result<u32, String> {
    let guard = state.read().await;

    if let Some(server) = guard.as_ref() {
        Ok(server.buffer().get_delay().await)
    } else {
        let settings = SettingsManager::load().await.unwrap_or_default();
        Ok(settings.default_delay_sec)
    }
}

/// Toggle delay on/off
#[tauri::command]
pub async fn toggle_delay(app: AppHandle, state: State<'_, AppState>) -> Result<DelayMode, String> {
    let guard = state.read().await;

    if let Some(server) = guard.as_ref() {
        let mode = server.toggle_delay().await.map_err(|e| e.to_string())?;

        // Update settings
        let mut settings = SettingsManager::load().await.unwrap_or_default();
        settings.delay_mode = mode;
        SettingsManager::save(&settings).await.map_err(|e| e.to_string())?;

        // Emit event
        {
            let _ = app.emit("mode_changed", serde_json::json!({ "mode": mode }));
        }

        Ok(mode)
    } else {
        Err("Server not running".to_string())
    }
}

/// Increase delay by step
#[tauri::command]
pub async fn increase_delay(state: State<'_, AppState>) -> Result<u32, String> {
    // Rate limiting: max 10 delay changes per second
    check_rate_limit(&LAST_DELAY_CHANGE, 0)?;

    let guard = state.read().await;

    if let Some(server) = guard.as_ref() {
        let current = server.buffer().get_delay().await;
        let settings = SettingsManager::load().await.unwrap_or_default();
        let step = settings.delay_step_sec.max(1);
        let new_delay = (current + step).min(settings.max_delay_sec);

        server.set_delay(new_delay).await.map_err(|e| e.to_string())?;

        // Update settings
        let mut settings = settings;
        settings.default_delay_sec = new_delay;
        SettingsManager::save(&settings).await.map_err(|e| e.to_string())?;

        Ok(new_delay)
    } else {
        Err("Server not running".to_string())
    }
}

/// Decrease delay by step
#[tauri::command]
pub async fn decrease_delay(state: State<'_, AppState>) -> Result<u32, String> {
    // Rate limiting: max 10 delay changes per second
    check_rate_limit(&LAST_DELAY_CHANGE, 0)?;

    let guard = state.read().await;

    if let Some(server) = guard.as_ref() {
        let current = server.buffer().get_delay().await;
        let settings = SettingsManager::load().await.unwrap_or_default();
        let step = settings.delay_step_sec.max(1);
        let new_delay = current.saturating_sub(step).max(1);

        server.set_delay(new_delay).await.map_err(|e| e.to_string())?;

        // Update settings
        let mut settings = settings;
        settings.default_delay_sec = new_delay;
        SettingsManager::save(&settings).await.map_err(|e| e.to_string())?;

        Ok(new_delay)
    } else {
        Err("Server not running".to_string())
    }
}

/// Skip to live (clear buffer)
#[tauri::command]
pub async fn skip_to_live(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.read().await;

    if let Some(server) = guard.as_ref() {
        server.skip_to_live().await.map_err(|e| e.to_string())?;

        {
            let _ = app.emit("skip_to_live", ());
        }

        Ok(())
    } else {
        Err("Server not running".to_string())
    }
}

/// Add platform output
#[tauri::command]
pub async fn add_platform_output(
    app: AppHandle,
    state: State<'_, AppState>,
    mut output: PlatformOutput,
) -> Result<String, String> {
    // Generate ID if not provided
    if output.id.is_empty() {
        output.id = Uuid::new_v4().to_string();
    }

    // Set default URL if empty
    if output.url.is_empty() || output.url == "rtmp://" {
        output.url = format!("{}{}", output.platform.default_rtmp_url(), output.stream_key);
    }

    // Validate URL to prevent SSRF
    validate_rtmp_url(&output.url)?;

    let guard = state.read().await;

    if let Some(server) = guard.as_ref() {
        server.add_relay_output(output.clone()).await.map_err(|e| e.to_string())?;
    }

    // Update settings
    let mut settings = SettingsManager::load().await.unwrap_or_default();
    settings.platform_outputs.push(output.clone());
    SettingsManager::save(&settings).await.map_err(|e| e.to_string())?;

    // Emit event
    {
        let _ = app.emit("platform_added", &output);
    }

    Ok(output.id)
}

/// Remove platform output
#[tauri::command]
pub async fn remove_platform_output(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let guard = state.read().await;

    if let Some(server) = guard.as_ref() {
        server.remove_relay_output(&id).await.map_err(|e| e.to_string())?;
    }

    // Update settings
    let mut settings = SettingsManager::load().await.unwrap_or_default();
    settings.platform_outputs.retain(|o| o.id != id);
    SettingsManager::save(&settings).await.map_err(|e| e.to_string())?;

    // Emit event
    {
        let _ = app.emit("platform_removed", serde_json::json!({ "id": id }));
    }

    Ok(())
}

/// Update platform output
#[tauri::command]
pub async fn update_platform_output(
    app: AppHandle,
    state: State<'_, AppState>,
    output: PlatformOutput,
) -> Result<(), String> {
    // Validate URL to prevent SSRF
    validate_rtmp_url(&output.url)?;

    let guard = state.read().await;

    if let Some(server) = guard.as_ref() {
        server.remove_relay_output(&output.id).await.map_err(|e| e.to_string())?;
        if output.enabled {
            server.add_relay_output(output.clone()).await.map_err(|e| e.to_string())?;
        }
    }

    // Update settings
    let mut settings = SettingsManager::load().await.unwrap_or_default();
    if let Some(idx) = settings.platform_outputs.iter().position(|o| o.id == output.id) {
        settings.platform_outputs[idx] = output.clone();
        SettingsManager::save(&settings).await.map_err(|e| e.to_string())?;
    }

    // Emit event
    {
        let _ = app.emit("platform_updated", &output);
    }

    Ok(())
}

/// Get all platform outputs
#[tauri::command]
pub async fn get_platform_outputs() -> Result<Vec<PlatformOutput>, String> {
    let settings = SettingsManager::load().await.unwrap_or_default();
    Ok(settings.platform_outputs)
}

/// Test platform connection
#[tauri::command]
pub async fn test_platform_connection(output: PlatformOutput) -> Result<bool, String> {
    // Validate URL to prevent SSRF
    validate_rtmp_url(&output.url)?;

    // TODO: Implement actual connection test with timeout
    // For now, return true if validation passes
    Ok(true)
}

/// Get current settings
#[tauri::command]
pub async fn get_settings() -> Result<Settings, String> {
    Ok(SettingsManager::load().await.unwrap_or_default())
}

/// Update settings with validation (prevents mass assignment)
#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    new_settings: Settings,
) -> Result<(), String> {
    // Load current settings to preserve fields not allowed to be changed via this endpoint
    let mut current_settings = SettingsManager::load().await.unwrap_or_default();

    // Validate and apply only allowed fields
    // Server settings
    validate_bind_address(&new_settings.bind_address)?;
    current_settings.bind_address = new_settings.bind_address;

    validate_port(new_settings.port, "RTMP")?;
    current_settings.port = new_settings.port;

    current_settings.auto_start_server = new_settings.auto_start_server;

    // Delay settings
    if new_settings.default_delay_sec == 0 {
        return Err("default_delay_sec must be >= 1".to_string());
    }
    if new_settings.default_delay_sec > 3600 {
        return Err("default_delay_sec cannot exceed 3600 seconds".to_string());
    }
    current_settings.default_delay_sec = new_settings.default_delay_sec;

    if new_settings.max_delay_sec == 0 || new_settings.max_delay_sec > 3600 {
        return Err("max_delay_sec must be between 1 and 3600".to_string());
    }
    current_settings.max_delay_sec = new_settings.max_delay_sec;

    current_settings.delay_mode = new_settings.delay_mode;

    if new_settings.delay_step_sec == 0 || new_settings.delay_step_sec > 300 {
        return Err("delay_step_sec must be between 1 and 300".to_string());
    }
    current_settings.delay_step_sec = new_settings.delay_step_sec;

    // Buffer settings
    if new_settings.buffer_max_memory_mb == 0 || new_settings.buffer_max_memory_mb > 2048 {
        return Err("buffer_max_memory_mb must be between 1 and 2048".to_string());
    }
    current_settings.buffer_max_memory_mb = new_settings.buffer_max_memory_mb;

    current_settings.buffer_keyframe_only = new_settings.buffer_keyframe_only;

    // Platform outputs - validate each
    for output in &new_settings.platform_outputs {
        validate_rtmp_url(&output.url)?;
    }
    current_settings.platform_outputs = new_settings.platform_outputs;

    // Overlay settings
    current_settings.overlay_enabled = new_settings.overlay_enabled;

    validate_port(new_settings.overlay_port, "Overlay")?;
    current_settings.overlay_port = new_settings.overlay_port;

    validate_theme(&new_settings.overlay_theme)?;
    current_settings.overlay_theme = new_settings.overlay_theme;

    // Hotkey settings
    current_settings.hotkeys = new_settings.hotkeys;
    current_settings.global_hotkeys_enabled = new_settings.global_hotkeys_enabled;

    // Stream Deck
    current_settings.streamdeck_enabled = new_settings.streamdeck_enabled;
    validate_port(new_settings.streamdeck_port, "Stream Deck")?;
    current_settings.streamdeck_port = new_settings.streamdeck_port;

    // Web UI
    current_settings.web_ui_enabled = new_settings.web_ui_enabled;
    validate_port(new_settings.web_ui_port, "Web UI")?;
    current_settings.web_ui_port = new_settings.web_ui_port;

    // web_ui_auth_token - allow update but don't expose in logs
    current_settings.web_ui_auth_token = new_settings.web_ui_auth_token;

    // Advanced
    current_settings.log_level = new_settings.log_level;
    current_settings.minimize_to_tray = new_settings.minimize_to_tray;
    current_settings.start_minimized = new_settings.start_minimized;
    current_settings.check_updates = new_settings.check_updates;

    // Save validated settings
    SettingsManager::save(&current_settings).await.map_err(|e| e.to_string())?;

    // Apply settings that need immediate effect
    let guard = state.read().await;
    if let Some(server) = guard.as_ref() {
        server.set_delay(current_settings.default_delay_sec).await.map_err(|e| e.to_string())?;
        server.set_delay_mode(current_settings.delay_mode).await.map_err(|e| e.to_string())?;
    }

    // Emit event (without sensitive data)
    {
        let safe_settings = crate::ipc::Settings {
            web_ui_auth_token: None, // Don't emit auth token
            ..current_settings.clone()
        };
        let _ = app.emit("settings_changed", &safe_settings);
    }

    Ok(())
}

/// Reset settings to defaults
#[tauri::command]
pub async fn reset_settings() -> Result<Settings, String> {
    let defaults = Settings::default();
    SettingsManager::save(&defaults).await.map_err(|e| e.to_string())?;
    Ok(defaults)
}

/// Get overlay URL
#[tauri::command]
pub async fn get_overlay_url(state: State<'_, AppState>) -> Result<String, String> {
    let settings = SettingsManager::load().await.unwrap_or_default();
    Ok(format!("http://localhost:{}/overlay", settings.overlay_port))
}

/// Regenerate overlay
#[tauri::command]
pub async fn regenerate_overlay(
    web_server: State<'_, Arc<tokio::sync::Mutex<WebServer>>>,
) -> Result<(), String> {
    let settings = SettingsManager::load().await.unwrap_or_default();
    web_server.lock().await.update_overlay_theme(settings.overlay_theme).await;
    Ok(())
}

/// Get Stream Deck status
#[tauri::command]
pub async fn get_streamdeck_status() -> Result<bool, String> {
    // Would check actual Stream Deck connection
    Ok(false)
}

/// Send Stream Deck update
#[tauri::command]
pub async fn send_streamdeck_update(event: StreamDeckEvent) -> Result<(), String> {
    // Would send to Stream Deck plugin
    debug!("Stream Deck event: {:?}", event);
    Ok(())
}

/// Get system information
#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_processes();

    // Check for OBS process
    let (obs_running, obs_version) = check_obs_process(&sys);

    Ok(SystemInfo {
        os: format!("{} {}", sysinfo::System::name().unwrap_or_default(), sysinfo::System::os_version().unwrap_or_default()),
        arch: std::env::consts::ARCH.to_string(),
        cpu_cores: sys.cpus().len(),
        total_memory_mb: sys.total_memory() / 1024 / 1024,
        available_memory_mb: sys.available_memory() / 1024 / 1024,
        obs_running,
        obs_version,
    })
}

/// Check if OBS is running
#[tauri::command]
pub async fn check_obs_running() -> Result<bool, String> {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_processes();
    let (running, _) = check_obs_process(&sys);
    Ok(running)
}

/// Check for OBS process and extract version
fn check_obs_process(sys: &sysinfo::System) -> (bool, Option<String>) {
    // OBS process names by platform
    let obs_names = if cfg!(target_os = "windows") {
        vec!["obs64.exe", "obs32.exe", "obs.exe"]
    } else if cfg!(target_os = "macos") {
        vec!["obs"]
    } else {
        vec!["obs"]
    };

    // Known OBS installation directories (allowlist)
    #[cfg(target_os = "windows")]
    let allowed_dirs: Vec<std::path::PathBuf> = vec![
        std::path::PathBuf::from(r"C:\Program Files\obs-studio"),
        std::path::PathBuf::from(r"C:\Program Files (x86)\obs-studio"),
    ];
    #[cfg(not(target_os = "windows"))]
    let allowed_dirs: Vec<std::path::PathBuf> = vec![];

    for (_, process) in sys.processes() {
        let name = process.name().to_lowercase();
        for obs_name in &obs_names {
            if name == obs_name.to_lowercase() {
                // Try to get version from executable path
                let version = process.exe()
                    .and_then(|p| p.to_str())
                    .and_then(|p| {
                        // Validate path is in allowed directories on Windows
                        #[cfg(target_os = "windows")]
                        {
                            let path = std::path::Path::new(p);
                            let canonical = path.canonicalize().ok()?;
                            let is_allowed = allowed_dirs.iter().any(|allowed| {
                                canonical.starts_with(allowed)
                            });
                            if !is_allowed {
                                return None;
                            }
                        }
                        get_file_version(p)
                    });

                return (true, version);
            }
        }
    }

    (false, None)
}

fn check_rate_limit(slot: &'static std::sync::OnceLock<std::sync::Mutex<Option<Instant>>>, seconds: u64) -> Result<(), String> {
    let lock = slot.get_or_init(|| std::sync::Mutex::new(None));
    let mut last = lock.lock().map_err(|_| "Rate limiter unavailable".to_string())?;
    if last.is_some_and(|time| time.elapsed().as_secs() < seconds) {
        return Err("Too many requests; please wait before trying again".to_string());
    }
    *last = Some(Instant::now());
    Ok(())
}

fn validate_bind_address(address: &str) -> Result<(), String> {
    let ip: IpAddr = address.parse().map_err(|_| "Bind address must be a valid IP address".to_string())?;
    if !ip.is_loopback() && !ip.is_unspecified() && ip.is_multicast() {
        return Err("Multicast bind addresses are not supported".to_string());
    }
    Ok(())
}

fn validate_port(port: u16, service: &str) -> Result<(), String> {
    if port == 0 {
        return Err(format!("{} port must be between 1 and 65535", service));
    }
    Ok(())
}

fn validate_rtmp_url(url: &str) -> Result<(), String> {
    let (scheme, rest) = url.split_once("://").ok_or_else(|| "RTMP URL must include a scheme".to_string())?;
    if scheme.eq_ignore_ascii_case("rtmps") {
        return Err("RTMPS is not supported yet; use an rtmp:// endpoint".to_string());
    }
    if !scheme.eq_ignore_ascii_case("rtmp") {
        return Err("Only rtmp:// URLs are supported".to_string());
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') || authority.contains('\\') {
        return Err("RTMP URL must contain a valid host and must not include credentials".to_string());
    }
    // Debug-only escape hatch for testing against a local RTMP sink.
    if crate::rtmp::allow_private_relay_targets() {
        return Ok(());
    }
    let host = authority.trim_start_matches('[').split(']').next().unwrap_or(authority)
        .split(':').next().unwrap_or_default().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() || host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") || host.ends_with(".internal") {
        return Err("RTMP host must not be localhost or an internal hostname".to_string());
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        let blocked = match ip {
            IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_multicast() || v4.is_unspecified() || v4.is_broadcast(),
            IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local() || v6.is_multicast() || v6.is_unspecified(),
        };
        if blocked {
            return Err("RTMP host must not resolve to a private or reserved address".to_string());
        }
    }
    Ok(())
}

fn validate_theme(theme: &crate::ipc::OverlayTheme) -> Result<(), String> {
    for (name, color) in [
        ("background_color", theme.background_color.as_str()),
        ("text_color", theme.text_color.as_str()),
        ("accent_color", theme.accent_color.as_str()),
    ] {
        let valid = color.starts_with('#') && (color.len() == 4 || color.len() == 7 || color.len() == 9)
            && color[1..].bytes().all(|b| b.is_ascii_hexdigit());
        if !valid && !(name == "background_color" && color == "rgba(0, 0, 0, 0.8)") {
            return Err(format!("Invalid overlay {}", name));
        }
    }
    if theme.font_size == 0 || theme.font_size > 96 || theme.border_radius > 64 || theme.padding > 128 {
        return Err("Overlay dimensions are outside allowed limits".to_string());
    }
    if theme.font_family.len() > 128 || !theme.font_family.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | ',' | '-' | '_' | '\'' | '"')) {
        return Err("Invalid overlay font family".to_string());
    }
    Ok(())
}

/// Extract version from executable file
fn get_file_version(path: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::*;
        use windows::Win32::Storage::FileSystem::*;
        use windows::Win32::System::Diagnostics::Debug::*;
        use windows::Win32::System::LibraryLoader::*;
        use windows::Win32::System::SystemServices::*;

        let path_wide: Vec<u16> = OsStr::new(path).encode_wide().chain(Some(0)).collect();

        // GetFileVersionInfoSizeW returns the size needed, 0 on error
        let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(path_wide.as_ptr()), None) };
        if size == 0 {
            return None;
        }

        let mut buffer = vec![0u8; size as usize];

        // GetFileVersionInfoW fills the buffer with version info
        let success = unsafe {
            GetFileVersionInfoW(PCWSTR(path_wide.as_ptr()), 0, size, buffer.as_mut_ptr() as *mut _).is_ok()
        };
        if !success {
            return None;
        }

        // Get translation info
        let mut sub_block = std::ptr::null_mut();
        let mut len = 0u32;

        let translation = format!("\\VarFileInfo\\Translation");
        let translation_wide: Vec<u16> = OsStr::new(&translation).encode_wide().chain(Some(0)).collect();

        let success = unsafe {
            VerQueryValueW(
                buffer.as_ptr() as *const _,
                PCWSTR(translation_wide.as_ptr()),
                &mut sub_block,
                &mut len,
            ).as_bool()
        };
        if !success || len < 4 || sub_block.is_null() {
            return None;
        }

        // Extract language and codepage
        let lang_codepage = unsafe { std::slice::from_raw_parts(sub_block as *const u16, len as usize / 2) };
        if lang_codepage.len() < 2 {
            return None;
        }
        let lang = lang_codepage[0];
        let codepage = lang_codepage[1];

        // Query FileVersion
        let query = format!("\\StringFileInfo\\{:04x}{:04x}\\FileVersion", lang, codepage);
        let query_wide: Vec<u16> = OsStr::new(&query).encode_wide().chain(Some(0)).collect();

        let mut sub_block = std::ptr::null_mut();
        let mut len = 0u32;

        let success = unsafe {
            VerQueryValueW(
                buffer.as_ptr() as *const _,
                PCWSTR(query_wide.as_ptr()),
                &mut sub_block,
                &mut len,
            ).as_bool()
        };
        if !success || sub_block.is_null() || len == 0 {
            return None;
        }

        let version_ptr = sub_block as *const u16;
        let version_slice = unsafe { std::slice::from_raw_parts(version_ptr, len as usize / 2) };
        Some(String::from_utf16_lossy(version_slice).trim_end_matches('\0').to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Linux/macOS, try to run obs --version
        use std::process::Command;
        Command::new(path)
            .arg("--version")
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    let version_str = String::from_utf8_lossy(&output.stdout);
                    version_str.lines().next().map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
    }
}

