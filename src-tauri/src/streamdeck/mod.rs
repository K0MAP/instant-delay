//! Stream Deck plugin integration

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn};

use crate::rtmp::SharedServer;
use crate::ipc::{AppState, DelayMode, StreamDeckEvent};

/// Stream Deck plugin manager
pub struct StreamDeckManager {
    state: AppState,
    connected: Arc<RwLock<bool>>,
    port: u16,
}

impl StreamDeckManager {
    /// Create new Stream Deck manager
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            connected: Arc::new(RwLock::new(false)),
            port: 3000, // Default Stream Deck plugin port
        }
    }

    /// Start Stream Deck connection
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // In a real implementation, this would:
        // 1. Connect to Stream Deck app via WebSocket
        // 2. Register plugin UUID
        // 3. Handle property inspector
        // 4. Send/receive events

        *self.connected.write().await = true;
        info!("Stream Deck plugin started on port {}", self.port);
        Ok(())
    }

    /// Stop Stream Deck connection
    pub async fn stop(&self) {
        *self.connected.write().await = false;
        info!("Stream Deck plugin stopped");
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }

    /// Send event to Stream Deck
    pub async fn send_event(&self, event: StreamDeckEvent) {
        if !*self.connected.read().await {
            return;
        }

        debug!("Sending Stream Deck event: {:?}", event);
        // Would send via WebSocket to Stream Deck app
    }

    /// Handle incoming Stream Deck command
    pub async fn handle_command(&self, command: StreamDeckCommand) -> StreamDeckResponse {
        let guard = self.state.read().await;

        if let Some(server) = guard.as_ref() {
            match command {
                StreamDeckCommand::GetStatus => {
                    let status = server.get_status().await;
                    let buffer_stats = status.buffer_stats.unwrap_or_default();

                    StreamDeckResponse::Status {
                        delay_sec: status.current_delay_sec,
                        delay_mode: status.delay_mode,
                        is_buffering: buffer_stats.is_buffering,
                        buffer_fill_percent: buffer_stats.fill_percent,
                        connected_outputs: status.relay_status.iter().filter(|r| r.connected).count(),
                        total_outputs: status.relay_status.len(),
                    }
                }
                StreamDeckCommand::SetDelay { seconds } => {
                    let _ = server.set_delay(seconds).await;
                    StreamDeckResponse::Ok
                }
                StreamDeckCommand::ToggleDelay => {
                    let mode = server.toggle_delay().await.unwrap_or(DelayMode::Instant);
                    StreamDeckResponse::ModeChanged { mode }
                }
                StreamDeckCommand::IncreaseDelay => {
                    let current = server.buffer().get_delay().await;
                    let new = (current + 5).min(600);
                    let _ = server.set_delay(new).await;
                    StreamDeckResponse::DelayChanged { seconds: new }
                }
                StreamDeckCommand::DecreaseDelay => {
                    let current = server.buffer().get_delay().await;
                    let new = current.saturating_sub(5).max(1);
                    let _ = server.set_delay(new).await;
                    StreamDeckResponse::DelayChanged { seconds: new }
                }
                StreamDeckCommand::SkipToLive => {
                    let _ = server.skip_to_live().await;
                    StreamDeckResponse::Ok
                }
                StreamDeckCommand::SetMode { mode } => {
                    let _ = server.set_delay_mode(mode).await;
                    StreamDeckResponse::ModeChanged { mode }
                }
            }
        } else {
            StreamDeckResponse::Error { message: "Server not running".to_string() }
        }
    }
}

/// Stream Deck commands from plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum StreamDeckCommand {
    GetStatus,
    SetDelay { seconds: u32 },
    ToggleDelay,
    IncreaseDelay,
    DecreaseDelay,
    SkipToLive,
    SetMode { mode: DelayMode },
}

/// Stream Deck responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum StreamDeckResponse {
    Status {
        delay_sec: u32,
        delay_mode: DelayMode,
        is_buffering: bool,
        buffer_fill_percent: f64,
        connected_outputs: usize,
        total_outputs: usize,
    },
    DelayChanged { seconds: u32 },
    ModeChanged { mode: DelayMode },
    Ok,
    Error { message: String },
}

/// Stream Deck manifest for plugin distribution
pub const STREAMDECK_MANIFEST: &str = r#"{
  "name": "InstantDelay",
  "description": "Control stream delay from Stream Deck",
  "version": "1.0.0",
  "author": "InstantDelay",
  "icon": "icon.png",
  "url": "https://github.com/K0MAP/instant-delay",
  "actions": [
    {
      "name": "Toggle Delay",
      "uuid": "com.instantdelay.toggle",
      "icon": "icons/toggle.png",
      "states": [
        {
          "title": "Toggle",
          "fontSize": 14
        }
      ],
      "supportedInMultiActions": true
    },
    {
      "name": "Increase Delay",
      "uuid": "com.instantdelay.increase",
      "icon": "icons/increase.png",
      "states": [
        {
          "title": "+5s",
          "fontSize": 14
        }
      ]
    },
    {
      "name": "Decrease Delay",
      "uuid": "com.instantdelay.decrease",
      "icon": "icons/decrease.png",
      "states": [
        {
          "title": "-5s",
          "fontSize": 14
        }
      ]
    },
    {
      "name": "Skip to Live",
      "uuid": "com.instantdelay.skip",
      "icon": "icons/skip.png",
      "states": [
        {
          "title": "LIVE",
          "fontSize": 14
        }
      ]
    },
    {
      "name": "Set Delay",
      "uuid": "com.instantdelay.setdelay",
      "icon": "icons/setdelay.png",
      "states": [
        {
          "title": "Delay",
          "fontSize": 14
        }
      ],
      "propertyInspectorPath": "property-inspector.html"
    },
    {
      "name": "Delay Status",
      "uuid": "com.instantdelay.status",
      "icon": "icons/status.png",
      "states": [
        {
          "title": "0s",
          "fontSize": 18
        }
      ],
      "propertyInspectorPath": "property-inspector.html"
    }
  ],
  "category": "Streaming",
  "categoryIcon": "icons/category.png"
}"#;

/// Property Inspector HTML for Stream Deck
pub const PROPERTY_INSPECTOR_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>InstantDelay Settings</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; padding: 20px; background: #2d2d2d; color: #fff; }
        .setting { margin-bottom: 16px; }
        label { display: block; margin-bottom: 4px; font-weight: 500; }
        input, select { width: 100%; padding: 8px; border: 1px solid #444; background: #3d3d3d; color: #fff; border-radius: 4px; }
        .btn { padding: 10px 20px; background: #ff6b35; border: none; color: white; border-radius: 4px; cursor: pointer; width: 100%; margin-top: 8px; }
        .btn:hover { background: #e55a2a; }
        .status { padding: 12px; background: #3d3d3d; border-radius: 4px; margin-bottom: 16px; }
        .status.connected { border-left: 4px solid #00ff00; }
        .status.disconnected { border-left: 4px solid #ff6b35; }
    </style>
</head>
<body>
    <div id="status" class="status disconnected">Connecting...</div>

    <div class="setting">
        <label>Default Delay (seconds)</label>
        <input type="number" id="defaultDelay" min="1" max="600" value="10">
    </div>

    <div class="setting">
        <label>Delay Step (seconds)</label>
        <input type="number" id="delayStep" min="1" max="60" value="5">
    </div>

    <div class="setting">
        <label>Default Mode</label>
        <select id="defaultMode">
            <option value="instant">Instant Mode</option>
            <option value="normal">Normal Mode</option>
            <option value="disabled">Disabled</option>
        </select>
    </div>

    <button class="btn" onclick="saveSettings()">Save Settings</button>

    <script>
        // Stream Deck API
        const { streamdeck } = window;

        streamdeck.on('willAppear', function(json) {
            updateStatus(json.payload.settings);
        });

        streamdeck.on('propertyInspectorDidAppear', function(json) {
            loadSettings();
        });

        function loadSettings() {
            streamdeck.propertyInspector.getPropertyInspectorSettings().then(settings => {
                if (settings) {
                    document.getElementById('defaultDelay').value = settings.defaultDelay || 10;
                    document.getElementById('delayStep').value = settings.delayStep || 5;
                    document.getElementById('defaultMode').value = settings.defaultMode || 'instant';
                }
            });
        }

        function saveSettings() {
            const settings = {
                defaultDelay: parseInt(document.getElementById('defaultDelay').value),
                delayStep: parseInt(document.getElementById('delayStep').value),
                defaultMode: document.getElementById('defaultMode').value
            };
            streamdeck.propertyInspector.setPropertyInspectorSettings(settings);
        }

        function updateStatus(settings) {
            const statusEl = document.getElementById('status');
            if (settings && settings.connected) {
                statusEl.textContent = 'Connected to InstantDelay';
                statusEl.className = 'status connected';
            } else {
                statusEl.textContent = 'Not connected to InstantDelay app';
                statusEl.className = 'status disconnected';
            }
        }
    </script>
</body>
</html>"#;