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
        let default_delay = config.read().await.default_delay_sec;
        let buffer = Arc::new(RingBuffer::new(buffer_config));
        // Keep the ring buffer's delay in sync with the server configuration.
        buffer.set_delay(default_delay).await;
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
        // When binding to port 0 the OS chooses the port; publish it so the
        // rest of the app (and tests) can report the real endpoint.
        let bound_port = listener.local_addr()?.port();
        if self.config.read().await.port == 0 {
            self.config.write().await.port = bound_port;
        }
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
        use crate::rtmp::protocol::server_handshake;

        // Perform the complete RTMP handshake (C0/C1 -> S0/S1 -> C2 -> S2)
        server_handshake(&mut stream).await?;

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
        _config: Arc<RwLock<ServerConfig>>,
    ) -> Result<()> {
        use crate::rtmp::protocol::{ChunkReader, ChunkWriter};

        let mut reader = ChunkReader::new();
        let mut writer = ChunkWriter::new();
        let mut drain_tick = interval(Duration::from_millis(50));
        // The destination cannot decode an H.264 stream that starts on a
        // non-keyframe, so hold back output until the first delayed keyframe.
        let mut output_started = false;
        // Acknowledgement bookkeeping (see Window Acknowledgement Size).
        let mut ack_window: u64 = 0;
        let mut bytes_received: u64 = 0;
        let mut bytes_acked: u64 = 0;

        loop {
            let message = tokio::select! {
                read = reader.read_message(&mut stream) => {
                    match read {
                        Ok(message) => message,
                        Err(e) => {
                            if e.to_string().contains("EOF") || e.to_string().contains("closed") {
                                debug!("Client disconnected");
                                break;
                            }
                            return Err(e);
                        }
                    }
                }
                _ = drain_tick.tick() => {
                    // Release delayed frames even when the source is quiet.
                    Self::relay_frames(
                        &session,
                        &relay_manager,
                        &stats,
                        &mut output_started,
                    ).await;
                    continue;
                }
            };

            // Update stats
            bytes_received += message.payload.len() as u64;
            {
                let mut stats_guard = stats.write().await;
                stats_guard.total_bytes_in += message.payload.len() as u64;
            }

            match message.header.message_type {
                MessageType::CommandAMF0 => {
                    if Self::handle_command(&mut stream, &mut writer, &message).await? {
                        break;
                    }
                }
                MessageType::Video | MessageType::Audio | MessageType::Metadata => {
                    {
                        let mut stats_guard = stats.write().await;
                        match message.header.message_type {
                            MessageType::Video => stats_guard.video_frames += 1,
                            MessageType::Audio => stats_guard.audio_frames += 1,
                            MessageType::Metadata => stats_guard.metadata_frames += 1,
                            _ => {}
                        }
                    }

                    let immediate = session.process_message(message).await?;
                    for msg in immediate {
                        relay_manager.relay_message(msg.clone()).await;
                        let mut stats_guard = stats.write().await;
                        stats_guard.total_bytes_out += msg.payload.len() as u64;
                    }

                    Self::relay_frames(
                        &session,
                        &relay_manager,
                        &stats,
                        &mut output_started,
                    ).await;
                }
                MessageType::SetChunkSize => {
                    // Applied inside ChunkReader.
                    debug!("Client chunk size is now {}", reader.chunk_size());
                }
                MessageType::WindowAcknowledgementSize => {
                    if message.payload.len() >= 4 {
                        ack_window = u32::from_be_bytes([
                            message.payload[0],
                            message.payload[1],
                            message.payload[2],
                            message.payload[3],
                        ]) as u64;
                        debug!("Client window acknowledgement size: {}", ack_window);
                    }
                }
                MessageType::UserControl => {
                    // Respond to PingRequest (event type 6) with PingResponse (7).
                    if message.payload.len() >= 6 {
                        let event_type = u16::from_be_bytes([message.payload[0], message.payload[1]]);
                        if event_type == 6 {
                            let mut payload = bytes::BytesMut::with_capacity(6);
                            payload.extend_from_slice(&7u16.to_be_bytes());
                            payload.extend_from_slice(&message.payload[2..6]);
                            let response = RtmpMessage {
                                header: crate::rtmp::protocol::ChunkHeader {
                                    chunk_stream_id: crate::rtmp::protocol::csid::PROTOCOL_CONTROL,
                                    timestamp: 0,
                                    message_length: 6,
                                    message_type: MessageType::UserControl,
                                    message_stream_id: 0,
                                },
                                payload: payload.freeze(),
                            };
                            writer.write_message(&mut stream, &response).await?;
                        }
                    }
                }
                MessageType::Abort
                | MessageType::Acknowledgement
                | MessageType::SetPeerBandwidth => {}
                _ => {}
            }

            // Acknowledge received bytes when the peer's window is reached.
            if ack_window > 0 && bytes_received.saturating_sub(bytes_acked) >= ack_window {
                let mut payload = bytes::BytesMut::with_capacity(4);
                payload.extend_from_slice(&((bytes_received & 0xFFFF_FFFF) as u32).to_be_bytes());
                let ack = RtmpMessage {
                    header: crate::rtmp::protocol::ChunkHeader {
                        chunk_stream_id: crate::rtmp::protocol::csid::PROTOCOL_CONTROL,
                        timestamp: 0,
                        message_length: 4,
                        message_type: MessageType::Acknowledgement,
                        message_stream_id: 0,
                    },
                    payload: payload.freeze(),
                };
                writer.write_message(&mut stream, &ack).await?;
                bytes_acked = bytes_received;
            }
        }

        // Keep the compiler aware that config is part of the connection state.
        let _ = &_config;
        Ok(())
    }

    /// Relay every frame whose delay has elapsed.
    async fn relay_frames(
        session: &Arc<crate::rtmp::session::Session>,
        relay_manager: &Arc<RelayManager>,
        stats: &Arc<RwLock<ServerStats>>,
        output_started: &mut bool,
    ) {
        for message in session.drain_ready().await {
            if !Self::should_emit(&message, output_started) {
                continue;
            }
            relay_manager.relay_message(message.clone()).await;
            let mut stats_guard = stats.write().await;
            stats_guard.total_bytes_out += message.payload.len() as u64;
        }
    }

    /// Drop everything until the first video keyframe so the destination can
    /// start decoding. Codec configuration packets (AVC/HEVC sequence headers)
    /// must still pass through: they arrive before the first keyframe and the
    /// destination needs them to decode it.
    fn should_emit(message: &RtmpMessage, output_started: &mut bool) -> bool {
        if *output_started {
            return true;
        }
        match message.header.message_type {
            MessageType::Video => {
                if message.payload.len() < 2 {
                    return false;
                }
                let codec = message.payload[0] & 0x0F;
                let is_keyframe = (message.payload[0] >> 4) == 1;
                match codec {
                    // AVC (7) / HEVC (12): byte 1 is the packet type.
                    7 | 12 => {
                        if message.payload[1] == 0 {
                            // Sequence header: send, but output has not started.
                            return true;
                        }
                        if message.payload[1] != 1 {
                            // End of sequence.
                            return false;
                        }
                        if is_keyframe {
                            *output_started = true;
                            true
                        } else {
                            false
                        }
                    }
                    _ => {
                        if is_keyframe {
                            *output_started = true;
                            true
                        } else {
                            false
                        }
                    }
                }
            }
            MessageType::Audio => false,
            _ => true,
        }
    }

    /// Handle an AMF0 command from the publishing client.
    /// Returns true when the connection should be closed.
    async fn handle_command(
        stream: &mut TcpStream,
        writer: &mut crate::rtmp::protocol::ChunkWriter,
        message: &RtmpMessage,
    ) -> Result<bool> {
        use crate::rtmp::protocol::{amf0, ChunkHeader};

        let Some((command, transaction_id)) = amf0::read_command(&message.payload) else {
            debug!("Received an AMF0 message that is not a command");
            return Ok(false);
        };

        debug!("RTMP command '{}' (transaction {})", command, transaction_id);

        let response = match command.as_str() {
            "connect" => Some(amf0::build_connect_result()),
            "createStream" => Some(amf0::build_create_stream_result(transaction_id, 1.0)),
            "publish" => Some(amf0::build_publish_start()),
            "releaseStream" | "FCPublish" | "_checkbw" | "getStreamLength" => {
                Some(amf0::build_simple_result(transaction_id))
            }
            "deleteStream" | "closeStream" => return Ok(true),
            _ => {
                if transaction_id != 0.0 {
                    Some(amf0::build_simple_result(transaction_id))
                } else {
                    None
                }
            }
        };

        if let Some(payload) = response {
            // onStatus must be sent on the message stream that was published.
            let message_stream_id = if command == "publish" { 1 } else { 0 };
            let response = RtmpMessage {
                header: ChunkHeader {
                    chunk_stream_id: crate::rtmp::protocol::csid::COMMAND,
                    timestamp: 0,
                    message_length: payload.len() as u32,
                    message_type: MessageType::CommandAMF0,
                    message_stream_id,
                },
                payload,
            };
            writer.write_message(stream, &response).await?;
        }

        Ok(false)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{Platform, PlatformOutput};
    use crate::rtmp::protocol::{
        amf0, csid, server_handshake, ChunkHeader, ChunkReader, ChunkWriter, RtmpMessage,
    };
    use bytes::{Bytes, BytesMut};
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Mutex as AsyncMutex;

    /// Minimal RTMP server standing in for a streaming platform.
    async fn run_sink(
        listener: TcpListener,
        records: Arc<AsyncMutex<Vec<(MessageType, Instant, usize)>>>,
    ) -> Result<()> {
        let (mut stream, _) = listener.accept().await?;
        server_handshake(&mut stream).await?;

        let mut reader = ChunkReader::new();
        let mut writer = ChunkWriter::new();

        loop {
            let message = match reader.read_message(&mut stream).await {
                Ok(message) => message,
                Err(_) => break,
            };

            if message.header.message_type != MessageType::CommandAMF0 {
                records.lock().await.push((
                    message.header.message_type,
                    Instant::now(),
                    message.payload.len(),
                ));
                continue;
            }

            let Some((command, transaction_id)) = amf0::read_command(&message.payload) else {
                continue;
            };
            let (payload, message_stream_id) = match command.as_str() {
                "connect" => (amf0::build_connect_result(), 0),
                "createStream" => (amf0::build_create_stream_result(transaction_id, 1.0), 0),
                "publish" => (amf0::build_publish_start(), 1),
                _ => (amf0::build_simple_result(transaction_id), 0),
            };
            let response = RtmpMessage {
                header: ChunkHeader {
                    chunk_stream_id: csid::COMMAND,
                    timestamp: 0,
                    message_length: payload.len() as u32,
                    message_type: MessageType::CommandAMF0,
                    message_stream_id,
                },
                payload,
            };
            writer.write_message(&mut stream, &response).await?;
        }

        Ok(())
    }

    async fn client_handshake(stream: &mut TcpStream) -> Result<()> {
        let mut c0c1 = [0u8; 1537];
        c0c1[0] = 3;
        stream.write_all(&c0c1).await?;
        stream.flush().await?;

        let mut s0s1 = [0u8; 1537];
        stream.read_exact(&mut s0s1).await?;
        if s0s1[0] != 3 {
            anyhow::bail!("unexpected RTMP version {}", s0s1[0]);
        }

        stream.write_all(&s0s1[1..]).await?;
        stream.flush().await?;

        let mut s2 = [0u8; 1536];
        stream.read_exact(&mut s2).await?;
        Ok(())
    }

    async fn send_command(
        stream: &mut TcpStream,
        writer: &mut ChunkWriter,
        payload: Bytes,
        message_stream_id: u32,
    ) -> Result<()> {
        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::COMMAND,
                timestamp: 0,
                message_length: payload.len() as u32,
                message_type: MessageType::CommandAMF0,
                message_stream_id,
            },
            payload,
        };
        writer.write_message(stream, &message).await
    }

    async fn wait_for_command(
        stream: &mut TcpStream,
        reader: &mut ChunkReader,
        expected: &str,
    ) -> Result<()> {
        for _ in 0..20 {
            let message = reader.read_message(stream).await?;
            if message.header.message_type != MessageType::CommandAMF0 {
                continue;
            }
            if let Some((command, _)) = amf0::read_command(&message.payload) {
                if command == expected {
                    return Ok(());
                }
            }
        }
        anyhow::bail!("did not receive '{}' from the RTMP server", expected)
    }

    fn media_message(message_type: MessageType, payload: Bytes) -> RtmpMessage {
        let chunk_stream_id = match message_type {
            MessageType::Video => csid::VIDEO,
            MessageType::Audio => csid::AUDIO,
            _ => csid::DATA,
        };
        RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id,
                timestamp: 0,
                message_length: payload.len() as u32,
                message_type,
                message_stream_id: 1,
            },
            payload,
        }
    }

    #[tokio::test]
    async fn accepts_a_publish_and_relays_it_after_the_configured_delay() {
        std::env::set_var("INSTANTDELAY_ALLOW_PRIVATE_RELAY", "1");

        // Destination standing in for Twitch/YouTube.
        let sink_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let sink_port = sink_listener.local_addr().unwrap().port();
        let records = Arc::new(AsyncMutex::new(Vec::new()));
        let sink_task = tokio::spawn(run_sink(sink_listener, records.clone()));

        let config = ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 0,
            max_connections: 4,
            buffer_config: BufferConfig {
                max_memory_mb: 64,
                ..Default::default()
            },
            relay_outputs: vec![PlatformOutput {
                id: "sink".to_string(),
                name: "Local sink".to_string(),
                platform: Platform::Custom,
                url: format!("rtmp://127.0.0.1:{}/live/test", sink_port),
                stream_key: "test".to_string(),
                enabled: true,
                priority: 0,
                backup: false,
            }],
            delay_mode: DelayMode::Normal,
            default_delay_sec: 1,
        };

        let mut server = RtmpServer::new(config).await.unwrap();
        server.start().await.unwrap();
        let server_addr: SocketAddr = format!("127.0.0.1:{}", server.get_status().await.port)
            .parse()
            .unwrap();

        // Publishing client (stands in for OBS).
        let mut publisher = TcpStream::connect(server_addr).await.unwrap();
        client_handshake(&mut publisher).await.unwrap();
        let mut reader = ChunkReader::new();
        let mut writer = ChunkWriter::new();

        let mut connect = BytesMut::new();
        amf0::write_connect_command(&mut connect, "live", "rtmp://127.0.0.1/live");
        send_command(&mut publisher, &mut writer, connect.freeze(), 0)
            .await
            .unwrap();
        wait_for_command(&mut publisher, &mut reader, "_result")
            .await
            .unwrap();

        let mut create_stream = BytesMut::new();
        amf0::write_create_stream_command(&mut create_stream);
        send_command(&mut publisher, &mut writer, create_stream.freeze(), 0)
            .await
            .unwrap();
        wait_for_command(&mut publisher, &mut reader, "_result")
            .await
            .unwrap();

        let mut publish = BytesMut::new();
        amf0::write_publish_command(&mut publish, "test", "live");
        send_command(&mut publisher, &mut writer, publish.freeze(), 0)
            .await
            .unwrap();
        wait_for_command(&mut publisher, &mut reader, "onStatus")
            .await
            .unwrap();

        // Metadata should pass through immediately.
        writer
            .write_message(
                &mut publisher,
                &media_message(MessageType::Metadata, Bytes::from_static(b"metadata")),
            )
            .await
            .unwrap();

        let publish_started = Instant::now();
        for frame in 0..30u8 {
            let message_type = if frame % 2 == 0 {
                MessageType::Video
            } else {
                MessageType::Audio
            };
            let payload = if frame == 0 {
                // Keyframe (frame type 1) so the destination can start decoding.
                Bytes::from_static(&[0x17, 0x01, 0x02, 0x03, 0x04])
            } else {
                Bytes::copy_from_slice(&[0x27, frame, 0x00, 0x00, 0x00])
            };
            writer
                .write_message(&mut publisher, &media_message(message_type, payload))
                .await
                .unwrap();
            if frame % 2 == 0 {
                tokio::time::sleep(Duration::from_millis(33)).await;
            }
        }

        // Wait for the delayed frames to appear at the destination.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let recorded = records.lock().await.clone();
            if recorded.iter().filter(|(kind, _, _)| *kind != MessageType::Metadata).count() >= 10
                || Instant::now() > deadline
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let recorded = records.lock().await.clone();
        let metadata_at = recorded
            .iter()
            .find(|(kind, _, _)| *kind == MessageType::Metadata)
            .map(|(_, at, _)| *at)
            .expect("destination did not receive metadata");

        let media: Vec<_> = recorded
            .iter()
            .filter(|(kind, _, _)| *kind == MessageType::Video || *kind == MessageType::Audio)
            .collect();

        assert!(!media.is_empty(), "destination did not receive media");
        assert!(
            metadata_at.duration_since(publish_started) < Duration::from_millis(500),
            "metadata should not be delayed"
        );

        let first_media_at = media.iter().map(|(_, at, _)| *at).min().unwrap();
        let first_media_delay = first_media_at.duration_since(publish_started);
        assert!(
            first_media_delay >= Duration::from_millis(800),
            "media arrived too early: {:?}",
            first_media_delay
        );
        assert!(
            first_media_delay < Duration::from_secs(4),
            "media arrived too late: {:?}",
            first_media_delay
        );

        let _ = server.stop().await;
        sink_task.abort();
        std::env::remove_var("INSTANTDELAY_ALLOW_PRIVATE_RELAY");
    }
}