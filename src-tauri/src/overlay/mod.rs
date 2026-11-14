//! Overlay generator for OBS browser source

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::ipc::OverlayTheme;
use tracing::debug;

/// Overlay generator
pub struct OverlayGenerator {
    theme: Arc<RwLock<OverlayTheme>>,
}

impl OverlayGenerator {
    /// Create new overlay generator
    pub fn new(theme: OverlayTheme) -> Self {
        Self {
            theme: Arc::new(RwLock::new(theme)),
        }
    }

    /// Generate overlay HTML
    pub async fn generate_html(&self) -> String {
        let theme = self.theme.read().await;
        Self::build_html(&theme)
    }

    /// Generate overlay HTML with custom data
    pub async fn generate_html_with_data(&self, data: &crate::ipc::OverlayData) -> String {
        let theme = self.theme.read().await;
        Self::build_html_with_data(&theme, data)
    }

    /// Build HTML string
    fn build_html(theme: &OverlayTheme) -> String {
        format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>InstantDelay Overlay</title>
    <style>
        {}
    </style>
</head>
<body>
    <div id="overlay" class="overlay-container">
        <div id="status-indicator" class="status-indicator"></div>
        <div id="status-text" class="status-text">CONNECTING...</div>
        <div id="delay-display" class="delay-display">
            <span id="delay-value">--</span>
            <span id="delay-unit">s</span>
        </div>
        <div id="buffer-bar" class="buffer-bar">
            <div id="buffer-fill" class="buffer-fill"></div>
        </div>
        <div id="platform-status" class="platform-status">
            <span id="platform-count">0/0</span>
        </div>
    </div>
    <script>
        {}
    </script>
</body>
</html>"#, Self::build_css(theme), Self::build_js())
    }

    /// Build HTML with data
    fn build_html_with_data(theme: &OverlayTheme, data: &crate::ipc::OverlayData) -> String {
        format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>InstantDelay Overlay</title>
    <style>
        {}
    </style>
</head>
<body>
    <div id="overlay" class="overlay-container">
        <div id="status-indicator" class="status-indicator" style="background-color: {};"></div>
        <div id="status-text" class="status-text">{}</div>
        <div id="delay-display" class="delay-display">
            <span id="delay-value">{}</span>
            <span id="delay-unit">s</span>
        </div>
        <div id="buffer-bar" class="buffer-bar">
            <div id="buffer-fill" class="buffer-fill" style="width: {}%; background-color: {};"></div>
        </div>
        <div id="platform-status" class="platform-status">
            <span id="platform-count">{}/{} connected</span>
        </div>
    </div>
    <script>
        {}
    </script>
</body>
</html>"#,
            Self::build_css(theme),
            data.status_color,
            data.status_text,
            data.delay_sec,
            data.buffer_fill_percent,
            data.status_color,
            data.connected_platforms,
            data.platform_count,
            Self::build_js()
        )
    }

    /// Build CSS
    fn build_css(theme: &OverlayTheme) -> String {
        format!(r#"
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}

        body {{
            font-family: {};
            background: transparent;
            overflow: hidden;
            width: 100vw;
            height: 100vh;
            display: flex;
            justify-content: center;
            align-items: center;
        }}

        .overlay-container {{
            position: relative;
            padding: {}px;
            border-radius: {}px;
            background-color: {};
            min-width: 200px;
            text-align: center;
            transition: all 0.3s ease;
        }}

        .status-indicator {{
            width: 12px;
            height: 12px;
            border-radius: 50%;
            margin: 0 auto 8px;
            transition: background-color 0.3s ease;
            box-shadow: 0 0 10px currentColor;
        }}

        .status-text {{
            font-size: {}px;
            font-weight: 600;
            color: {};
            text-transform: uppercase;
            letter-spacing: 1px;
            margin-bottom: 8px;
            transition: color 0.3s ease;
        }}

        .delay-display {{
            font-size: {}px;
            font-weight: 700;
            color: {};
            margin-bottom: 12px;
            font-variant-numeric: tabular-nums;
        }}

        .delay-display #delay-value {{
            font-size: 1.5em;
        }}

        .delay-display #delay-unit {{
            font-size: 0.6em;
            opacity: 0.8;
            margin-left: 4px;
        }}

        .buffer-bar {{
            width: 100%;
            height: 6px;
            background-color: rgba(255, 255, 255, 0.1);
            border-radius: 3px;
            overflow: hidden;
            margin-bottom: 8px;
        }}

        .buffer-fill {{
            height: 100%;
            border-radius: 3px;
            transition: width 0.5s ease, background-color 0.3s ease;
        }}

        .platform-status {{
            font-size: 11px;
            color: rgba(255, 255, 255, 0.6);
            font-family: monospace;
        }}

        @keyframes pulse {{
            0%, 100% {{ opacity: 1; }}
            50% {{ opacity: 0.5; }}
        }}

        .buffering .status-indicator {{
            animation: pulse 1s infinite;
        }}

        .live .status-indicator {{
            animation: none;
        }}

        /* Reduced motion */
        @media (prefers-reduced-motion: reduce) {{
            * {{
                animation: none !important;
                transition: none !important;
            }}
        }}
        "#,
            theme.font_family,
            theme.padding,
            theme.border_radius,
            theme.background_color,
            theme.font_size,
            theme.text_color,
            theme.font_size + 8,
            theme.text_color,
        )
    }

    /// Build JavaScript
    fn build_js() -> String {
        r#"
        // WebSocket connection for real-time updates
        let ws = null;
        let reconnectAttempts = 0;
        const maxReconnectAttempts = 10;
        const reconnectDelay = 1000;

        function connect() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            const wsUrl = `${protocol}//${window.location.host}/overlay/ws`;

            ws = new WebSocket(wsUrl);

            ws.onopen = function() {
                console.log('Overlay WebSocket connected');
                reconnectAttempts = 0;
            };

            ws.onmessage = function(event) {
                try {
                    const data = JSON.parse(event.data);
                    updateOverlay(data);
                } catch (e) {
                    console.error('Failed to parse overlay data:', e);
                }
            };

            ws.onclose = function() {
                console.log('Overlay WebSocket disconnected');
                if (reconnectAttempts < maxReconnectAttempts) {
                    reconnectAttempts++;
                    setTimeout(connect, reconnectDelay * reconnectAttempts);
                }
            };

            ws.onerror = function(error) {
                console.error('Overlay WebSocket error:', error);
            };
        }

        function updateOverlay(data) {
            // Update status indicator
            const indicator = document.getElementById('status-indicator');
            const statusText = document.getElementById('status-text');
            const delayValue = document.getElementById('delay-value');
            const bufferFill = document.getElementById('buffer-fill');
            const platformCount = document.getElementById('platform-count');
            const overlay = document.getElementById('overlay');

            if (data.is_buffering) {
                overlay.classList.add('buffering');
                overlay.classList.remove('live');
            } else {
                overlay.classList.add('live');
                overlay.classList.remove('buffering');
            }

            indicator.style.backgroundColor = data.status_color;
            statusText.textContent = data.status_text;
            delayValue.textContent = data.delay_sec;
            bufferFill.style.width = data.buffer_fill_percent + '%';
            bufferFill.style.backgroundColor = data.status_color;
            platformCount.textContent = `${data.connected_platforms}/${data.platform_count} connected`;
        }

        // Initial connection
        connect();

        // Fallback: poll REST endpoint if WebSocket fails
        setInterval(async function() {
            if (ws && ws.readyState === WebSocket.OPEN) return;

            try {
                const response = await fetch('/overlay/data');
                const data = await response.json();
                updateOverlay(data);
            } catch (e) {
                // Silently fail
            }
        }, 2000);
        "#.to_string()
    }

    /// Update theme
    pub async fn update_theme(&self, theme: OverlayTheme) {
        *self.theme.write().await = theme;
    }

    /// Get current theme
    pub async fn get_theme(&self) -> OverlayTheme {
        self.theme.read().await.clone()
    }
}