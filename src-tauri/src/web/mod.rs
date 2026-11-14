//! Web server for overlay, remote control, and WebSocket API

use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{
    Router,
    routing::{get, post},
    extract::{State, Query, WebSocketUpgrade, Path},
    response::{Html, IntoResponse, Response},
    Json,
    http::{StatusCode, header, Method, HeaderValue},
};
use axum::extract::ws::{Message, WebSocket};
use tower_http::cors::CorsLayer;
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn, error};
use uuid::Uuid;

use crate::rtmp::SharedServer;
use crate::ipc::{AppState, DelayMode, OverlayData};
use crate::overlay::OverlayGenerator;

/// Web server state
#[derive(Clone)]
pub struct WebServerState {
    pub rtmp_server: SharedServer,
    pub app_state: AppState,
    pub overlay_generator: Arc<OverlayGenerator>,
    pub auth_token: Option<String>,
}

/// Web server
pub struct WebServer {
    state: WebServerState,
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl WebServer {
    /// Create new web server
    pub fn new(state: AppState, settings: &crate::ipc::Settings) -> Self {
        let overlay_generator = Arc::new(OverlayGenerator::new(settings.overlay_theme.clone()));

        Self {
            state: WebServerState {
                rtmp_server: state.clone(),
                app_state: state,
                overlay_generator,
                auth_token: settings.web_ui_auth_token.clone(),
            },
            server_handle: None,
        }
    }

    /// Start web server
    pub async fn start(&mut self) -> anyhow::Result<()> {
        let settings = crate::config::SettingsManager::load().await.unwrap_or_default();
        let port = settings.web_ui_port;
        let overlay_port = settings.overlay_port;

        let app = self.create_router().await;

        // Main web UI server
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
        info!("Web UI server started on http://127.0.0.1:{}", port);

        // Overlay server (separate port for OBS browser source)
        let overlay_app = self.create_overlay_router().await;
        let overlay_listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", overlay_port)).await?;
        info!("Overlay server started on http://127.0.0.1:{}", overlay_port);

        let state = self.state.clone();

        self.server_handle = Some(tokio::spawn(async move {
            // Run both servers
            let main_server = axum::serve(listener, app);
            let overlay_server = axum::serve(overlay_listener, overlay_app);

            tokio::select! {
                res = main_server => {
                    if let Err(e) = res {
                        error!("Main web server error: {}", e);
                    }
                }
                res = overlay_server => {
                    if let Err(e) = res {
                        error!("Overlay web server error: {}", e);
                    }
                }
            }
        }));

        Ok(())
    }

    pub async fn update_overlay_theme(&self, theme: crate::ipc::OverlayTheme) {
        self.state.overlay_generator.update_theme(theme).await;
    }

    /// Stop web server
    pub async fn stop(&mut self) {
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }
    }

    /// Create main API router
    async fn create_router(&self) -> Router {
        // Restrict CORS to Tauri origins only
        let cors = CorsLayer::new()
            .allow_origin(HeaderValue::from_static("tauri://localhost"))
            .allow_origin(HeaderValue::from_static("https://tauri.localhost"))
            .allow_origin(HeaderValue::from_static("http://localhost:8080"))
            .allow_origin(HeaderValue::from_static("http://127.0.0.1:8080"))
            .allow_origin(HeaderValue::from_static("http://localhost:8081"))
            .allow_origin(HeaderValue::from_static("http://127.0.0.1:8081"))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(true);

        Router::new()
            // Health check
            .route("/health", get(health_check))
            // API routes
            .route("/api/status", get(api_status))
            .route("/api/delay", get(api_get_delay).post(api_set_delay))
            .route("/api/delay/toggle", post(api_toggle_delay))
            .route("/api/delay/increase", post(api_increase_delay))
            .route("/api/delay/decrease", post(api_decrease_delay))
            .route("/api/delay/skip", post(api_skip_to_live))
            .route("/api/mode", get(api_get_mode).post(api_set_mode))
            .route("/api/platforms", get(api_get_platforms).post(api_add_platform))
            .route("/api/platforms/:id", get(api_get_platform).put(api_update_platform).delete(api_remove_platform))
            .route("/api/settings", get(api_get_settings).put(api_update_settings))
            .route("/api/hotkeys", get(api_get_hotkeys).put(api_update_hotkeys))
            .route("/api/system", get(api_system_info))
            // WebSocket for real-time updates
            .route("/ws", get(websocket_handler))
            .layer(cors)
            .with_state(self.state.clone())
    }

    /// Create overlay router
    async fn create_overlay_router(&self) -> Router {
        // Restrict CORS for overlay to OBS browser source origins
        let cors = CorsLayer::new()
            .allow_origin(HeaderValue::from_static("http://localhost:8080"))
            .allow_origin(HeaderValue::from_static("http://127.0.0.1:8080"))
            .allow_origin(HeaderValue::from_static("tauri://localhost"))
            .allow_origin(HeaderValue::from_static("https://tauri.localhost"))
            .allow_methods([Method::GET])
            .allow_headers([header::CONTENT_TYPE]);

        Router::new()
            .route("/overlay", get(overlay_html))
            .route("/overlay/data", get(overlay_data))
            .route("/overlay/ws", get(overlay_websocket))
            .layer(cors)
            .with_state(self.state.clone())
    }
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Get server status
async fn api_status(State(state): State<WebServerState>) -> impl IntoResponse {
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        let status = server.get_status().await;
        Json(status).into_response()
    } else {
        Json(serde_json::json!({ "running": false })).into_response()
    }
}

/// Get current delay
async fn api_get_delay(State(state): State<WebServerState>) -> impl IntoResponse {
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        Json(serde_json::json!({ "delay_sec": server.buffer().get_delay().await })).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "Server not running" }))).into_response()
    }
}

/// Set delay
async fn api_set_delay(
    State(state): State<WebServerState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let seconds = payload.get("seconds").and_then(|v| v.as_u64()).unwrap_or(10) as u32;

    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        if let Err(e) = server.set_delay(seconds).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response();
        }
        Json(serde_json::json!({ "delay_sec": seconds })).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "Server not running" }))).into_response()
    }
}

/// Toggle delay
async fn api_toggle_delay(State(state): State<WebServerState>) -> impl IntoResponse {
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        match server.toggle_delay().await {
            Ok(mode) => Json(serde_json::json!({ "mode": mode })).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
        }
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "Server not running" }))).into_response()
    }
}

/// Increase delay
async fn api_increase_delay(State(state): State<WebServerState>) -> impl IntoResponse {
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        let current = server.buffer().get_delay().await;
        let new = (current + 5).min(600);
        let _ = server.set_delay(new).await;
        Json(serde_json::json!({ "delay_sec": new })).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "Server not running" }))).into_response()
    }
}

/// Decrease delay
async fn api_decrease_delay(State(state): State<WebServerState>) -> impl IntoResponse {
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        let current = server.buffer().get_delay().await;
        let new = current.saturating_sub(5).max(1);
        let _ = server.set_delay(new).await;
        Json(serde_json::json!({ "delay_sec": new })).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "Server not running" }))).into_response()
    }
}

/// Skip to live
async fn api_skip_to_live(State(state): State<WebServerState>) -> impl IntoResponse {
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        let _ = server.skip_to_live().await;
        Json(serde_json::json!({ "success": true })).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "Server not running" }))).into_response()
    }
}

/// Get delay mode
async fn api_get_mode(State(state): State<WebServerState>) -> impl IntoResponse {
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        let status = server.get_status().await;
        Json(serde_json::json!({ "mode": status.delay_mode })).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "Server not running" }))).into_response()
    }
}

/// Set delay mode
async fn api_set_mode(
    State(state): State<WebServerState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mode_str = payload.get("mode").and_then(|v| v.as_str()).unwrap_or("instant");
    let mode = match mode_str {
        "instant" => DelayMode::Instant,
        "normal" => DelayMode::Normal,
        "disabled" => DelayMode::Disabled,
        _ => DelayMode::Instant,
    };

    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        let _ = server.set_delay_mode(mode).await;
        Json(serde_json::json!({ "mode": mode })).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "Server not running" }))).into_response()
    }
}

/// Get platforms
async fn api_get_platforms(State(state): State<WebServerState>) -> impl IntoResponse {
    let settings = crate::config::SettingsManager::load().await.unwrap_or_default();
    Json(settings.platform_outputs)
}

/// Add platform
async fn api_add_platform(
    State(state): State<WebServerState>,
    Json(mut platform): Json<crate::ipc::PlatformOutput>,
) -> impl IntoResponse {
    if platform.id.is_empty() {
        platform.id = Uuid::new_v4().to_string();
    }

    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        if let Err(e) = server.add_relay_output(platform.clone()).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response();
        }
    }

    // Update settings
    let mut settings = crate::config::SettingsManager::load().await.unwrap_or_default();
    settings.platform_outputs.push(platform.clone());
    let _ = crate::config::SettingsManager::save(&settings).await;

    Json(platform).into_response()
}

/// Get platform
async fn api_get_platform(
    State(_state): State<WebServerState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let settings = crate::config::SettingsManager::load().await.unwrap_or_default();
    if let Some(platform) = settings.platform_outputs.iter().find(|p| p.id == id) {
        Json(platform.clone()).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response()
    }
}

/// Update platform
async fn api_update_platform(
    State(state): State<WebServerState>,
    Path(id): Path<String>,
    Json(platform): Json<crate::ipc::PlatformOutput>,
) -> impl IntoResponse {
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        let _ = server.remove_relay_output(&id).await;
        if platform.enabled {
            let _ = server.add_relay_output(platform.clone()).await;
        }
    }

    // Update settings
    let mut settings = crate::config::SettingsManager::load().await.unwrap_or_default();
    if let Some(idx) = settings.platform_outputs.iter().position(|p| p.id == id) {
        settings.platform_outputs[idx] = platform.clone();
        let _ = crate::config::SettingsManager::save(&settings).await;
    }

    Json(platform).into_response()
}

/// Remove platform
async fn api_remove_platform(
    State(state): State<WebServerState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        let _ = server.remove_relay_output(&id).await;
    }

    let mut settings = crate::config::SettingsManager::load().await.unwrap_or_default();
    settings.platform_outputs.retain(|p| p.id != id);
    let _ = crate::config::SettingsManager::save(&settings).await;

    Json(serde_json::json!({ "success": true })).into_response()
}

/// Get settings
async fn api_get_settings() -> impl IntoResponse {
    let settings = crate::config::SettingsManager::load().await.unwrap_or_default();
    Json(settings)
}

/// Update settings
async fn api_update_settings(
    State(state): State<WebServerState>,
    Json(new_settings): Json<crate::ipc::Settings>,
) -> impl IntoResponse {
    let _ = crate::config::SettingsManager::save(&new_settings).await;

    // Apply to running server
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        let _ = server.set_delay(new_settings.default_delay_sec).await;
        let _ = server.set_delay_mode(new_settings.delay_mode).await;
    }

    Json(serde_json::json!({ "success": true })).into_response()
}

/// Get hotkeys
async fn api_get_hotkeys() -> impl IntoResponse {
    let settings = crate::config::SettingsManager::load().await.unwrap_or_default();
    Json(settings.hotkeys)
}

/// Update hotkeys
async fn api_update_hotkeys(
    Json(hotkeys): Json<crate::ipc::HotkeyConfig>,
) -> impl IntoResponse {
    let mut settings = crate::config::SettingsManager::load().await.unwrap_or_default();
    settings.hotkeys = hotkeys;
    let _ = crate::config::SettingsManager::save(&settings).await;
    Json(serde_json::json!({ "success": true })).into_response()
}

/// System info
async fn api_system_info() -> impl IntoResponse {
    let sys = sysinfo::System::new_all();
    Json(serde_json::json!({
        "os": format!("{} {}", sysinfo::System::name().unwrap_or_default(), sysinfo::System::os_version().unwrap_or_default()),
        "arch": std::env::consts::ARCH,
        "cpu_cores": sys.cpus().len(),
        "total_memory_mb": sys.total_memory() / 1024 / 1024,
        "available_memory_mb": sys.available_memory() / 1024 / 1024,
    }))
}

/// WebSocket handler for real-time updates
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<WebServerState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

async fn handle_websocket(mut socket: WebSocket, state: WebServerState) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let guard = state.rtmp_server.read().await;
                if let Some(server) = guard.as_ref() {
                    let status = server.get_status().await;
                    let buffer_stats = status.buffer_stats.unwrap_or_default();

                    let data = OverlayData {
                        delay_sec: status.current_delay_sec,
                        delay_mode: status.delay_mode,
                        is_buffering: buffer_stats.is_buffering,
                        buffer_fill_percent: buffer_stats.fill_percent,
                        status_text: if buffer_stats.is_buffering { "BUFFERING" } else { "LIVE" }.to_string(),
                        status_color: if buffer_stats.is_buffering { "#ff6b35" } else { "#00ff00" }.to_string(),
                        platform_count: status.relay_status.len(),
                        connected_platforms: status.relay_status.iter().filter(|r| r.connected).count(),
                    };

                    if socket.send(Message::Text(serde_json::to_string(&data).unwrap())).await.is_err() {
                        break;
                    }
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Handle incoming messages (commands)
                        debug!("WS received: {}", text);
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Err(e)) => {
                        warn!("WS error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Overlay HTML page
async fn overlay_html(State(state): State<WebServerState>) -> Html<String> {
    let html = state.overlay_generator.generate_html().await;
    Html(html)
}

/// Overlay data endpoint
async fn overlay_data(State(state): State<WebServerState>) -> Json<OverlayData> {
    let guard = state.rtmp_server.read().await;
    if let Some(server) = guard.as_ref() {
        let status = server.get_status().await;
        let buffer_stats = status.buffer_stats.unwrap_or_default();

        Json(OverlayData {
            delay_sec: status.current_delay_sec,
            delay_mode: status.delay_mode,
            is_buffering: buffer_stats.is_buffering,
            buffer_fill_percent: buffer_stats.fill_percent,
            status_text: if buffer_stats.is_buffering { "BUFFERING" } else { "LIVE" }.to_string(),
            status_color: if buffer_stats.is_buffering { "#ff6b35" } else { "#00ff00" }.to_string(),
            platform_count: status.relay_status.len(),
            connected_platforms: status.relay_status.iter().filter(|r| r.connected).count(),
        })
    } else {
        Json(OverlayData {
            delay_sec: 0,
            delay_mode: DelayMode::Disabled,
            is_buffering: false,
            buffer_fill_percent: 0.0,
            status_text: "OFFLINE".to_string(),
            status_color: "#888888".to_string(),
            platform_count: 0,
            connected_platforms: 0,
        })
    }
}

/// Overlay WebSocket
async fn overlay_websocket(
    ws: WebSocketUpgrade,
    State(state): State<WebServerState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_overlay_websocket(socket, state))
}

async fn handle_overlay_websocket(mut socket: WebSocket, state: WebServerState) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let guard = state.rtmp_server.read().await;
                if let Some(server) = guard.as_ref() {
                    let status = server.get_status().await;
                    let buffer_stats = status.buffer_stats.unwrap_or_default();

                    let data = OverlayData {
                        delay_sec: status.current_delay_sec,
                        delay_mode: status.delay_mode,
                        is_buffering: buffer_stats.is_buffering,
                        buffer_fill_percent: buffer_stats.fill_percent,
                        status_text: if buffer_stats.is_buffering { "BUFFERING" } else { "LIVE" }.to_string(),
                        status_color: if buffer_stats.is_buffering { "#ff6b35" } else { "#00ff00" }.to_string(),
                        platform_count: status.relay_status.len(),
                        connected_platforms: status.relay_status.iter().filter(|r| r.connected).count(),
                    };

                    if socket.send(Message::Text(serde_json::to_string(&data).unwrap())).await.is_err() {
                        break;
                    }
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}