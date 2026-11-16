//! RTMP Server implementation for stream delay

pub mod server;
pub mod session;
pub mod buffer;
pub mod relay;
pub mod protocol;

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use crate::ipc::{PlatformOutput, DelayMode};

pub use server::RtmpServer;
pub use buffer::{RingBuffer, BufferConfig, BufferStats};
pub use relay::RelayManager;
pub use session::SessionManager;

/// RTMP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_address: String,
    pub port: u16,
    pub max_connections: usize,
    pub buffer_config: BufferConfig,
    pub relay_outputs: Vec<PlatformOutput>,
    pub delay_mode: DelayMode,
    pub default_delay_sec: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 1935,
            max_connections: 10,
            buffer_config: BufferConfig::default(),
            relay_outputs: Vec::new(),
            delay_mode: DelayMode::Instant,
            default_delay_sec: 10,
        }
    }
}

/// Whether private/loopback relay destinations are permitted.
///
/// This exists only for local end-to-end testing against an on-machine RTMP
/// sink. It is compiled out of release builds and must be opted into at
/// runtime with `INSTANTDELAY_ALLOW_PRIVATE_RELAY=1`.
pub fn allow_private_relay_targets() -> bool {
    cfg!(debug_assertions)
        && std::env::var("INSTANTDELAY_ALLOW_PRIVATE_RELAY")
            .map(|value| value == "1")
            .unwrap_or(false)
}

/// Server status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    pub running: bool,
    pub bind_address: String,
    pub port: u16,
    pub active_connections: usize,
    pub total_bytes_in: u64,
    pub total_bytes_out: u64,
    pub uptime_sec: u64,
    pub current_delay_sec: u32,
    pub delay_mode: DelayMode,
    pub buffer_stats: Option<BufferStats>,
    pub relay_status: Vec<RelayStatus>,
}

pub use relay::RelayStatus;

/// Shared server state for access from commands
pub type SharedServer = Arc<RwLock<Option<RtmpServer>>>;

/// Initialize RTMP server with config
pub async fn create_server(config: ServerConfig) -> anyhow::Result<RtmpServer> {
    RtmpServer::new(config).await
}

/// Get server status
pub async fn get_server_status(server: &SharedServer) -> ServerStatus {
    let guard = server.read().await;
    if let Some(server) = guard.as_ref() {
        server.get_status().await
    } else {
        ServerStatus {
            running: false,
            bind_address: "127.0.0.1".to_string(),
            port: 1935,
            active_connections: 0,
            total_bytes_in: 0,
            total_bytes_out: 0,
            uptime_sec: 0,
            current_delay_sec: 0,
            delay_mode: DelayMode::Instant,
            buffer_stats: None,
            relay_status: Vec::new(),
        }
    }
}