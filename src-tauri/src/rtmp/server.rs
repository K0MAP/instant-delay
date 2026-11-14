//! RTMP Server - Main server implementation

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, mpsc, broadcast};
use tokio::time::interval;
use tracing::{info, warn, error, debug, instrument};
use bytes::BytesMut;
use anyhow::Result;

use crate::rtmp::{ServerConfig, ServerStatus, RelayStatus, SharedServer};
use crate::rtmp::session::{SessionManager, SessionConfig, IncomingStream};
use crate::rtmp::buffer::{RingBuffer, BufferConfig};
use crate::rtmp::relay::RelayManager;
use crate::rtmp::protocol::{RtmpMessage, ChunkStreamId, MessageType};
use crate::ipc::{DelayMode, PlatformOutput};

/// RTMP Server state
pub struct RtmpServer {
    config: Arc<RwLock<ServerConfig>>,
    listener: Option<Arc<TcpListener>>,
    session_manager: Arc<SessionManager>,
    relay_manager: Arc<RelayManager>,
    buffer: Arc<RingBuffer>,
    stats: Arc<RwLock<ServerStats>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
    start_time: Instant,
}

/// Server statistics
#[derive(Debug, Default, Clone)]
pub struct ServerStats {
    pub total_connections: u64,
    pub active_connections: usize,
    pub total_bytes_in: u64,
    pub total_bytes_out: u64,
    pub video_frames: u64,
    pub audio_frames: u64,
    pub metadata_frames: u64,
    pub errors: u64,
}

impl RtmpServer {
    /// Create new RTMP server
    #[instrument]
    pub async fn new(config: ServerConfig) -> Result<Self> {
        let config = Arc::new(RwLock::new(config));
        let buffer_config = config.read().await.buffer_config.clone();
        let buffer = Arc::new(RingBuffer::new(buffer_config));
        let session_manager = Arc::new(SessionManager::new(buffer.clone()));
        let relay_manager = Arc::new(RelayManager::new());
        let stats = Arc::new(RwLock::new(ServerStats::default()));

        // Initialize relay outputs from config
        {
            let config_guard = config.read().await;
            for output in &config_guard.relay_outputs {
                if output.enabled {
                    relay_manager.add_output(output.clone()).await?;
                }
            }
        }

        Ok(Self {
            config,
            listener: None,
            session_manager,
            relay_manager,
            buffer,
            stats,
            shutdown_tx: None,
            start_time: Instant::now(),
        })
    }

    /// Start the RTMP server
    #[instrument(skip(self))]
    pub async fn start(&mut self) -> Result<()> {
        let config = self.config.read().await;
        let addr = format!("{}:{}", config.bind_address, config.port);
        drop(config);

        let listener = Arc::new(TcpListener::bind(&addr).await?);
        info!("RTMP server listening on {}", addr);

        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx);
        self.listener = Some(listener.clone());

        let session_manager = self.session_manager.clone();
        let relay_manager = self.relay_manager.clone();
        let buffer = self.buffer.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();

        // Accept loop
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("RTMP server shutdown signal received");
                        break;
                    }
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                debug!("New connection from {}", addr);
                                let session_manager = session_manager.clone();
                                let relay_manager = relay_manager.clone();
                                let buffer = buffer.clone();
                                let stats = stats.clone();
                                let config = config.clone();

                                tokio::spawn(async move {
                                    if let Err(e) = Self::handle_connection(
                                        stream, addr, session_manager, relay_manager, buffer, stats, config
                                    ).await {
                                        error!("Connection error from {}: {}", addr, e);
                                    }
                                });
                            }
                            Err(e) => {
                                error!("Accept error: {}", e);
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                    }
                }
            }
        });

        // Start periodic stats update
        self.start_stats_updater().await;

        Ok(())
    }

    /// Stop the RTMP server
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Close all sessions
        self.session_manager.close_all().await;

        // Stop all relays
        self.relay_manager.stop_all().await;

        // Close listener
        self.listener = None;

        info!("RTMP server stopped");
        Ok(())
    }

    /// Handle incoming RTMP connection
    async fn handle_connection(
        mut stream: TcpStream,
        addr: std::net::SocketAddr,
        session_manager: Arc<SessionManager>,
        relay_manager: Arc<RelayManager>,
        buffer: Arc<RingBuffer>,
        stats: Arc<RwLock<ServerStats>>,
        config: Arc<RwLock<ServerConfig>>,
    ) -> Result<()> {
        use crate::rtmp::protocol::{read_handshake, write_handshake, HandshakeType};

        // Perform RTMP handshake
        let _client_type = read_handshake(&mut stream).await?;
        write_handshake(&mut stream, HandshakeType::Server).await?;

        debug!("Handshake complete with {}", addr);

        // Create session config
        let session_config = {
            let config_guard = config.read().await;
            SessionConfig {
                default_delay_sec: config_guard.default_delay_sec,
                delay_mode: config_guard.delay_mode.clone(),
                buffer_config: config_guard.buffer_config.clone(),
            }
        };

        // Create session
        let session = session_manager.create_session(addr, session_config, buffer.clone()).await?;

        // Update stats
        {
            let mut stats_guard = stats.write().await;
            stats_guard.total_connections += 1;
            stats_guard.active_connections += 1;
        }

        // Handle RTMP messages
        let result = Self::process_rtmp_messages(
            stream, session, relay_manager, stats.clone(), config
        ).await;

        // Cleanup
        {
            let mut stats_guard = stats.write().await;
            stats_guard.active_connections = stats_guard.active_connections.saturating_sub(1);
        }
        session_manager.remove_session(&addr).await;

        result
    }

    /// Process RTMP messages from client
    async fn process_rtmp_messages(
        mut stream: TcpStream,
        session: Arc<crate::rtmp::session::Session>,
        relay_manager: Arc<RelayManager>,
        stats: Arc<RwLock<ServerStats>>,
        config: Arc<RwLock<ServerConfig>>,
    ) -> Result<()> {
        use crate::rtmp::protocol::{read_chunk, ChunkHeader, ChunkStreamId};
        use crate::rtmp::session::SessionEvent;

        loop {
            let chunk = match read_chunk(&mut stream).await {
                Ok(chunk) => chunk,
                Err(e) => {
                    if e.to_string().contains("EOF") || e.to_string().contains("closed") {
                        debug!("Client disconnected");
                        break;
                    }
                    return Err(e);
                }
            };

            // Update stats
            {
                let mut stats_guard = stats.write().await;
                stats_guard.total_bytes_in += chunk.payload.len() as u64;
            }

            // The current protocol reader returns a message directly.
            // Stateful RTMP chunk reassembly is not implemented by that reader.
            {
                let message = chunk;
                // Complete message received
                let message_type = message.header.message_type;

                // Update frame stats
                {
                    let mut stats_guard = stats.write().await;
                    match message_type {
                        MessageType::Video => stats_guard.video_frames += 1,
                        MessageType::Audio => stats_guard.audio_frames += 1,
                        MessageType::Metadata => stats_guard.metadata_frames += 1,
                        _ => {}
                    }
                }

                // Process through session (handles buffering/delay)
                let processed = session.process_message(message).await?;

                // Relay to outputs if not in delay buffering phase
                if let Some(msg) = processed {
                    relay_manager.relay_message(msg).await;

                    // Update output stats
                    {
                        let mut stats_guard = stats.write().await;
                        stats_guard.total_bytes_out += 1; // approximate
                    }
                }
            }
        }

        Ok(())
    }

    /// Start periodic statistics updater
    async fn start_stats_updater(&self) {
        let buffer = self.buffer.clone();
        let stats = self.stats.clone();
        let relay_manager = self.relay_manager.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));
            loop {
                interval.tick().await;

                // Update buffer stats
                let buffer_stats = buffer.get_stats().await;

                // Update relay stats
                let relay_status = relay_manager.get_status().await;

                // Could emit events here for UI updates
                debug!("Buffer: {:?}, Relays: {}", buffer_stats, relay_status.len());
            }
        });
    }

    /// Get current server status
    #[instrument(skip(self))]
    pub async fn get_status(&self) -> ServerStatus {
        let config = self.config.read().await;
        let stats = self.stats.read().await;
        let buffer_stats = self.buffer.get_stats().await;
        let relay_status = self.relay_manager.get_status().await;

        ServerStatus {
            running: self.listener.is_some(),
            bind_address: config.bind_address.clone(),
            port: config.port,
            active_connections: stats.active_connections,
            total_bytes_in: stats.total_bytes_in,
            total_bytes_out: stats.total_bytes_out,
            uptime_sec: self.start_time.elapsed().as_secs(),
            current_delay_sec: config.default_delay_sec,
            delay_mode: config.delay_mode.clone(),
            buffer_stats: Some(buffer_stats),
            relay_status,
        }
    }

    /// Update delay configuration
    pub async fn set_delay(&self, delay_sec: u32) -> Result<()> {
        let mut config = self.config.write().await;
        config.default_delay_sec = delay_sec.clamp(0, 600);
        self.buffer.set_delay(config.default_delay_sec).await;
        self.session_manager.update_delay(config.default_delay_sec).await;
        info!("Delay set to {} seconds", config.default_delay_sec);
        Ok(())
    }

    /// Toggle delay mode
    pub async fn toggle_delay(&self) -> Result<DelayMode> {
        let mut config = self.config.write().await;
        config.delay_mode = match config.delay_mode {
            DelayMode::Instant => DelayMode::Normal,
            DelayMode::Normal => DelayMode::Instant,
            DelayMode::Disabled => DelayMode::Instant,
        };
        self.session_manager.set_delay_mode(config.delay_mode.clone()).await;
        info!("Delay mode changed to {:?}", config.delay_mode);
        Ok(config.delay_mode.clone())
    }

    /// Set delay mode explicitly
    pub async fn set_delay_mode(&self, mode: DelayMode) -> Result<()> {
        let mut config = self.config.write().await;
        config.delay_mode = mode.clone();
        self.session_manager.set_delay_mode(mode).await;
        Ok(())
    }

    /// Add relay output
    pub async fn add_relay_output(&self, output: PlatformOutput) -> Result<()> {
        let mut config = self.config.write().await;
        config.relay_outputs.push(output.clone());
        drop(config);
        self.relay_manager.add_output(output).await
    }

    /// Remove relay output
    pub async fn remove_relay_output(&self, id: &str) -> Result<()> {
        let mut config = self.config.write().await;
        config.relay_outputs.retain(|o| o.id != id);
        drop(config);
        self.relay_manager.remove_output(id).await
    }

    /// Skip to live (clear buffer)
    pub async fn skip_to_live(&self) -> Result<()> {
        self.buffer.clear().await;
        self.session_manager.skip_to_live().await;
        info!("Skipped to live");
        Ok(())
    }

    /// Get buffer reference for external access
    pub fn buffer(&self) -> Arc<RingBuffer> {
        self.buffer.clone()
    }

    /// Get session manager reference
    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
    }

    /// Get relay manager reference
    pub fn relay_manager(&self) -> Arc<RelayManager> {
        self.relay_manager.clone()
    }
}

impl Drop for RtmpServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}