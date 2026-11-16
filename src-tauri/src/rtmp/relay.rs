//! Multi-output relay manager for streaming to multiple platforms

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex, mpsc, broadcast};
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use std::io::Read;
use tokio::time::{sleep, timeout};
use tracing::{info, warn, error, debug, instrument};
use bytes::{Bytes, BytesMut, BufMut};
use anyhow::{Result, anyhow};
use uuid::Uuid;

use crate::rtmp::protocol::{RtmpMessage, ChunkHeader, MessageType, write_chunk, ChunkReader, ChunkWriter, csid, amf0};
use std::io::Seek;
use crate::ipc::PlatformOutput;

// For cryptographically secure random
use rand::rngs::OsRng;
use rand::RngCore;

/// Relay output connection state
struct RelayConnection {
    output: PlatformOutput,
    stream: Option<TcpStream>,
    connected: bool,
    connecting: bool,
    last_error: Option<String>,
    bytes_sent: u64,
    connected_at: Option<Instant>,
    message_tx: mpsc::Sender<RelayMessage>,
    chunk_size: u32,
    stream_id: u32,
    reconnect_attempts: u32,
    last_ping: Instant,
    // Stateful chunk writer to maintain per-CSID state across messages
    chunk_writer: ChunkWriter,
}

/// Messages for relay connection
#[derive(Debug)]
enum RelayMessage {
    Connect,
    Disconnect,
    SendMessage(RtmpMessage),
    UpdateOutput(PlatformOutput),
    SetChunkSize(u32),
}

/// Relay status for UI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelayStatus {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub url: String,
    pub connected: bool,
    pub connecting: bool,
    pub bytes_sent: u64,
    pub uptime_sec: u64,
    pub last_error: Option<String>,
}

/// Configuration for reconnection
const MAX_RECONNECT_ATTEMPTS: u32 = 10;
const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(60);
const PING_INTERVAL: Duration = Duration::from_secs(30);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);

/// Relay manager for multiple outputs
pub struct RelayManager {
    connections: Arc<RwLock<HashMap<String, Arc<Mutex<RelayConnection>>>>>,
    message_queues: Arc<RwLock<HashMap<String, mpsc::Sender<RelayMessage>>>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl RelayManager {
    /// Create new relay manager
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            message_queues: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
        }
    }

    /// Initialize with shutdown signal
    pub fn set_shutdown(&mut self, tx: broadcast::Sender<()>) {
        self.shutdown_tx = Some(tx);
    }

    /// Add a new relay output
    #[instrument(skip(self, output))]
    pub async fn add_output(&self, output: PlatformOutput) -> Result<()> {
        if !output.enabled {
            return Ok(());
        }

        let id = output.id.clone();

        // Create message channel
        let (tx, mut rx) = mpsc::channel::<RelayMessage>(1000);

        // Create connection state
        let connection = Arc::new(Mutex::new(RelayConnection {
            output: output.clone(),
            stream: None,
            connected: false,
            connecting: false,
            last_error: None,
            bytes_sent: 0,
            connected_at: None,
            message_tx: tx.clone(),
            chunk_size: 128,
            stream_id: 0,
            reconnect_attempts: 0,
            last_ping: Instant::now(),
            chunk_writer: ChunkWriter::new(),
        }));

        // Store connection and queue
        self.connections.write().await.insert(id.clone(), connection.clone());
        self.message_queues.write().await.insert(id.clone(), tx.clone());

        // Connect only after the receiver task is running.
        if tx.send(RelayMessage::Connect).await.is_err() {
            warn!("Relay {} could not request an initial connection", id);
        }

        // Spawn connection handler
        let conn_clone = connection.clone();
        let shutdown_rx = self.shutdown_tx.as_ref().map(|tx| tx.subscribe());
        tokio::spawn(async move {
            Self::run_connection_handler(conn_clone, rx, shutdown_rx).await;
        });

        info!("Added relay output: {} ({})", output.name, output.platform);
        Ok(())
    }

    /// Remove a relay output
    pub async fn remove_output(&self, id: &str) -> Result<()> {
        // Send disconnect message
        if let Some(tx) = self.message_queues.write().await.remove(id) {
            let _ = tx.send(RelayMessage::Disconnect).await;
        }

        // Remove connection
        self.connections.write().await.remove(id);

        info!("Removed relay output: {}", id);
        Ok(())
    }

    /// Update relay output configuration
    pub async fn update_output(&self, output: PlatformOutput) -> Result<()> {
        if let Some(tx) = self.message_queues.read().await.get(&output.id) {
            tx.send(RelayMessage::UpdateOutput(output)).await?;
        }
        Ok(())
    }

    /// Relay a message to all connected outputs
    #[instrument(skip(self, message))]
    pub async fn relay_message(&self, message: RtmpMessage) {
        let queues = self.message_queues.read().await;
        for (id, tx) in queues.iter() {
            if tx.send(RelayMessage::SendMessage(message.clone())).await.is_err() {
                debug!("Failed to queue message for relay {}", id);
            }
        }
    }

    /// Get status of all relays
    pub async fn get_status(&self) -> Vec<RelayStatus> {
        let connections = self.connections.read().await;
        let mut statuses = Vec::new();

        for (id, conn) in connections.iter() {
            let conn_guard = conn.lock().await;
            let output = &conn_guard.output;

            statuses.push(RelayStatus {
                id: id.clone(),
                name: output.name.clone(),
                platform: output.platform.to_string(),
                url: Self::sanitize_url(&output.url),
                connected: conn_guard.connected,
                connecting: conn_guard.connecting,
                bytes_sent: conn_guard.bytes_sent,
                uptime_sec: conn_guard.connected_at.map(|t| t.elapsed().as_secs()).unwrap_or(0),
                last_error: conn_guard.last_error.clone(),
            });
        }

        statuses
    }

    /// Stop all relays
    pub async fn stop_all(&self) {
        let ids: Vec<String> = self.connections.read().await.keys().cloned().collect();
        for id in ids {
            let _ = self.remove_output(&id).await;
        }
    }

    /// Sanitize URL for display (hide stream key)
    fn sanitize_url(url: &str) -> String {
        if let Some(idx) = url.rfind('/') {
            let (base, key) = url.split_at(idx + 1);
            if key.len() > 4 {
                format!("{}****{}", base, &key[key.len()-4..])
            } else {
                format!("{}****", base)
            }
        } else {
            url.to_string()
        }
    }

    /// Connection handler loop with reconnection logic
    async fn run_connection_handler(
        connection: Arc<Mutex<RelayConnection>>,
        mut rx: mpsc::Receiver<RelayMessage>,
        mut shutdown_rx: Option<broadcast::Receiver<()>>,
    ) {
        let mut ping_interval = tokio::time::interval(PING_INTERVAL);
        let mut first_connect = true;

        loop {
            tokio::select! {
                _ = async {
                    if let Some(ref mut shutdown) = shutdown_rx {
                        shutdown.recv().await.ok();
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    info!("Relay received shutdown signal");
                    Self::disconnect_internal(connection.clone()).await;
                    break;
                }
                msg = rx.recv() => {
                    match msg {
                        Some(RelayMessage::Connect) => {
                            if first_connect { first_connect = false; }
                            Self::connect_internal(connection.clone()).await;
                        }
                        Some(RelayMessage::Disconnect) => {
                            Self::disconnect_internal(connection.clone()).await;
                            break;
                        }
                        Some(RelayMessage::SendMessage(message)) => {
                            Self::send_message_internal(connection.clone(), message).await;
                        }
                        Some(RelayMessage::UpdateOutput(output)) => {
                            Self::update_output_internal(connection.clone(), output).await;
                        }
                        Some(RelayMessage::SetChunkSize(size)) => {
                            Self::set_chunk_size_internal(connection.clone(), size).await;
                        }
                        None => break,
                    }
                }
                _ = ping_interval.tick() => {
                    Self::send_ping(connection.clone()).await;
                }
            }
        }
    }

    /// Spawn background task to read and handle server control messages
    fn spawn_reader_task(connection: Arc<Mutex<RelayConnection>>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut reader = ChunkReader::new();
            let mut stream = {
                let mut conn = connection.lock().await;
                conn.stream.take()
            };

            let Some(mut stream) = stream else {
                return;
            };

            loop {
                match reader.read_message(&mut stream).await {
                    Ok(message) => {
                        match message.header.message_type {
                            MessageType::WindowAcknowledgementSize => {
                                debug!("Received WindowAcknowledgementSize");
                                // Could track and send acknowledgement if needed
                            }
                            MessageType::SetPeerBandwidth => {
                                debug!("Received SetPeerBandwidth");
                            }
                            MessageType::SetChunkSize => {
                                if message.payload.len() >= 4 {
                                    let size = u32::from_be_bytes([
                                        message.payload[0],
                                        message.payload[1],
                                        message.payload[2],
                                        message.payload[3],
                                    ]);
                                    debug!("Server set chunk size to {}", size);
                                    let mut conn = connection.lock().await;
                                    conn.chunk_writer.set_chunk_size(size);
                                }
                            }
                            MessageType::UserControl => {
                                if message.payload.len() >= 6 {
                                    let event_type = u16::from_be_bytes([message.payload[0], message.payload[1]]);
                                    if event_type == 6 { // PingRequest
                                        // Respond with PingResponse (event type 7)
                                        let mut payload = bytes::BytesMut::with_capacity(6);
                                        payload.extend_from_slice(&7u16.to_be_bytes());
                                        payload.extend_from_slice(&message.payload[2..6]);
                                        let response = RtmpMessage {
                                            header: ChunkHeader {
                                                chunk_stream_id: csid::PROTOCOL_CONTROL,
                                                timestamp: 0,
                                                message_length: 6,
                                                message_type: MessageType::UserControl,
                                                message_stream_id: 0,
                                            },
                                            payload: payload.freeze(),
                                        };
                                        // Send ping response using chunk_writer - use separate borrows
                                        let mut conn = connection.lock().await;
                                        // Take stream out temporarily to avoid double borrow
                                        let mut stream_opt = conn.stream.take();
                                        let writer = &mut conn.chunk_writer;
                                        if let Some(ref mut stream) = stream_opt {
                                            let _ = writer.write_message(stream, &response).await;
                                        }
                                        // Put stream back
                                        conn.stream = stream_opt;
                                    }
                                }
                            }
                            MessageType::Acknowledgement => {
                                debug!("Received Acknowledgement");
                            }
                            _ => {
                                debug!("Received unexpected message type: {:?}", message.header.message_type);
                            }
                        }
                    }
                    Err(e) => {
                        if e.to_string().contains("EOF") || e.to_string().contains("closed") {
                            debug!("Server closed connection");
                        } else {
                            debug!("Reader error: {}", e);
                        }
                        break;
                    }
                }
            }

            // Put stream back (will be None since connection is dead)
            let mut conn = connection.lock().await;
            conn.stream = Some(stream);
        })
    }

    /// Internal connect implementation with full RTMP handshake
    async fn connect_internal(connection: Arc<Mutex<RelayConnection>>) {
        let (url, stream_key) = {
            let mut conn = connection.lock().await;
            if conn.connected || conn.connecting {
                return;
            }
            conn.connecting = true;
            conn.last_error = None;
            (conn.output.url.clone(), conn.output.stream_key.clone())
        };

        // Parse RTMP URL
        let (host, port, app, stream_name) = match Self::parse_rtmp_url(&url, &stream_key) {
            Ok(parsed) => parsed,
            Err(e) => {
                {
                    let mut conn = connection.lock().await;
                    conn.connecting = false;
                    conn.last_error = Some(format!("Invalid URL: {}", e));
                }
                error!("Relay URL parse error: {}", e);
                Self::schedule_reconnect(connection).await;
                return;
            }
        };

        // Connect TCP with timeout
        let addr = format!("{}:{}", host, port);
        let mut stream = match timeout(CONNECTION_TIMEOUT, TcpStream::connect(&addr)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                {
                    let mut conn = connection.lock().await;
                    conn.connecting = false;
                    conn.last_error = Some(format!("Connection failed: {}", e));
                }
                error!("Relay connection failed to {}: {}", addr, e);
                Self::schedule_reconnect(connection).await;
                return;
            }
            Err(_) => {
                {
                    let mut conn = connection.lock().await;
                    conn.connecting = false;
                    conn.last_error = Some("Connection timeout".to_string());
                }
                error!("Relay connection timeout to {}", addr);
                Self::schedule_reconnect(connection).await;
                return;
            }
        };

        // Set TCP_NODELAY for low latency
        if let Err(e) = stream.set_nodelay(true) {
            debug!("Failed to set TCP_NODELAY: {}", e);
        }

        // Perform RTMP handshake (C0/C1/C2)
        if let Err(e) = Self::perform_handshake(&mut stream).await {
            {
                let mut conn = connection.lock().await;
                conn.connecting = false;
                conn.last_error = Some(format!("Handshake failed: {}", e));
            }
            error!("Relay handshake failed: {}", e);
            Self::schedule_reconnect(connection).await;
            return;
        }

        // One reader for the whole connection: RTMP chunk headers are stateful.
        let mut reader = ChunkReader::new();

        // Send connect command using stateful chunk writer
        let tc_url = format!("rtmp://{}:{}/{}", host, port, app);
        if let Err(e) = Self::send_connect_command_internal(connection.clone(), &mut stream, &app, &tc_url).await {
            {
                let mut conn = connection.lock().await;
                conn.connecting = false;
                conn.last_error = Some(format!("Connect command failed: {}", e));
            }
            error!("Relay connect command failed: {}", e);
            Self::schedule_reconnect(connection).await;
            return;
        }

        // Read connect response and handle window acknowledgment
        let chunk_size = match Self::read_connect_response(&mut stream, &mut reader).await {
            Ok(result) => result,
            Err(e) => {
                {
                    let mut conn = connection.lock().await;
                    conn.connecting = false;
                    conn.last_error = Some(format!("Connect response failed: {}", e));
                }
                error!("Relay connect response failed: {}", e);
                Self::schedule_reconnect(connection).await;
                return;
            }
        };

        // Update chunk writer with negotiated size
        {
            let mut conn = connection.lock().await;
            conn.chunk_writer.set_chunk_size(chunk_size);
        }

        // Send setChunkSize if server sent different size
        if chunk_size != 128 {
            if let Err(e) = Self::send_set_chunk_size_internal(connection.clone(), &mut stream, chunk_size).await {
                warn!("Failed to set chunk size: {}", e);
            }
        }

        // Send createStream
        if let Err(e) = Self::send_create_stream_internal(connection.clone(), &mut stream).await {
            {
                let mut conn = connection.lock().await;
                conn.connecting = false;
                conn.last_error = Some(format!("createStream failed: {}", e));
            }
            error!("Relay createStream failed: {}", e);
            Self::schedule_reconnect(connection).await;
            return;
        }

        // Read createStream response to get stream ID
        let stream_id = match Self::read_create_stream_response(&mut stream, &mut reader).await {
            Ok(id) => id,
            Err(e) => {
                {
                    let mut conn = connection.lock().await;
                    conn.connecting = false;
                    conn.last_error = Some(format!("createStream response failed: {}", e));
                }
                error!("Relay createStream response failed: {}", e);
                Self::schedule_reconnect(connection).await;
                return;
            }
        };

        // Send publish
        if let Err(e) = Self::send_publish_internal(connection.clone(), &mut stream, &stream_name, stream_id).await {
            {
                let mut conn = connection.lock().await;
                conn.connecting = false;
                conn.last_error = Some(format!("Publish failed: {}", e));
            }
            error!("Relay publish failed: {}", e);
            Self::schedule_reconnect(connection).await;
            return;
        }

        // Read publish response (onStatus)
        if let Err(e) = Self::read_publish_response(&mut stream, &mut reader).await {
            {
                let mut conn = connection.lock().await;
                conn.connecting = false;
                conn.last_error = Some(format!("Publish response failed: {}", e));
            }
            error!("Relay publish response failed: {}", e);
            Self::schedule_reconnect(connection).await;
            return;
        }

        // Success!
        {
            let mut conn = connection.lock().await;
            conn.stream = Some(stream);
            conn.connected = true;
            conn.connecting = false;
            conn.connected_at = Some(Instant::now());
            conn.bytes_sent = 0;
            conn.chunk_size = chunk_size;
            conn.chunk_writer.set_chunk_size(chunk_size);
            conn.stream_id = stream_id;
            conn.reconnect_attempts = 0;
            conn.last_ping = Instant::now();
        }

        info!("Relay connected: {} -> {}", connection.lock().await.output.name, addr);
    }

    /// Perform RTMP handshake (client side: C0/C1/C2)
    async fn perform_handshake(stream: &mut TcpStream) -> Result<()> {
        // Send C0 + C1
        let mut c0c1 = [0u8; 1537];
        c0c1[0] = 3; // Version

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u32;
        c0c1[1..5].copy_from_slice(&now.to_be_bytes());
        c0c1[5..9].copy_from_slice(&[0, 0, 0, 0]); // Zero padding

        // Random data for C1 (1528 bytes) - use cryptographically secure RNG
        OsRng.fill_bytes(&mut c0c1[9..]);

        stream.write_all(&c0c1).await?;
        stream.flush().await?;

        // Read S0 + S1 (1537 bytes)
        let mut s0s1 = [0u8; 1537];
        stream.read_exact(&mut s0s1).await?;

        // Verify S0 version
        if s0s1[0] != 3 {
            return Err(anyhow!("Unsupported RTMP version: {}", s0s1[0]));
        }

        // Send C2 (copy of S1)
        stream.write_all(&s0s1[1..1537]).await?;
        stream.flush().await?;

        // Consume S2 if the server sends it.
        let mut s2 = [0u8; 1536];
        stream.read_exact(&mut s2).await?;

        Ok(())
    }

    /// Send connect command with AMF0 encoding
    async fn send_connect_command(stream: &mut TcpStream, app: &str, tc_url: &str) -> Result<()> {
        let mut payload = BytesMut::new();
        amf0::write_connect_command(&mut payload, app, tc_url);

        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::COMMAND,
                timestamp: 0,
                message_length: payload.len() as u32,
                message_type: MessageType::CommandAMF0,
                message_stream_id: 0,
            },
            payload: payload.freeze(),
        };

        write_chunk(stream, &message).await
    }

    /// Read connect response and handle protocol control messages
    async fn read_connect_response(stream: &mut TcpStream, reader: &mut ChunkReader) -> Result<u32> {
        let mut got_result = false;

        // Read messages until we get _result for connect
        for _ in 0..20 { // Limit to prevent infinite loop
            let message = reader.read_message(stream).await?;

            match message.header.message_type {
                MessageType::SetChunkSize => {
                    // ChunkReader has already applied the new chunk size.
                    debug!("Server set chunk size to {}", reader.chunk_size());
                }
                MessageType::WindowAcknowledgementSize => {
                    // Server window ack size - ignore for client
                    debug!("Server window ack size");
                }
                MessageType::SetPeerBandwidth => {
                    // Server peer bandwidth - ignore
                    debug!("Server peer bandwidth");
                }
                MessageType::CommandAMF0 => {
                    // Parse _result or _error
                    if let Some((cmd, _trans_id)) = Self::parse_command_response(&message.payload) {
                        if cmd == "_result" {
                            got_result = true;
                            break;
                        } else if cmd == "_error" {
                            return Err(anyhow!("Connect failed: server returned _error"));
                        }
                    }
                }
                _ => {}
            }
        }

        if !got_result {
            return Err(anyhow!("Did not receive _result for connect"));
        }

        Ok(reader.chunk_size())
    }

    /// Send setChunkSize command
    async fn send_set_chunk_size(stream: &mut TcpStream, size: u32) -> Result<()> {
        let mut payload = BytesMut::with_capacity(4);
        payload.put_u32(size);

        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::PROTOCOL_CONTROL,
                timestamp: 0,
                message_length: 4,
                message_type: MessageType::SetChunkSize,
                message_stream_id: 0,
            },
            payload: payload.freeze(),
        };

        write_chunk(stream, &message).await
    }

    /// Send createStream command
    async fn send_create_stream(stream: &mut TcpStream) -> Result<()> {
        let mut payload = BytesMut::new();
        amf0::write_create_stream_command(&mut payload);

        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::COMMAND,
                timestamp: 0,
                message_length: payload.len() as u32,
                message_type: MessageType::CommandAMF0,
                message_stream_id: 0,
            },
            payload: payload.freeze(),
        };

        write_chunk(stream, &message).await
    }

    /// Read createStream response to get stream ID
    async fn read_create_stream_response(stream: &mut TcpStream, reader: &mut ChunkReader) -> Result<u32> {
        for _ in 0..10 {
            let message = reader.read_message(stream).await?;

            if message.header.message_type == MessageType::CommandAMF0 {
                if let Some((cmd, trans_id)) = Self::parse_command_response(&message.payload) {
                    if cmd == "_result" && trans_id == 2.0 {
                        // Parse stream ID from response
                        // Response format: _result, transaction_id, null, stream_id
                        return Ok(Self::parse_stream_id(&message.payload).unwrap_or(1));
                    } else if cmd == "_error" {
                        return Err(anyhow!("createStream failed"));
                    }
                }
            }
        }
        Ok(1) // Default fallback
    }

    /// Send publish command
    async fn send_publish(stream: &mut TcpStream, stream_name: &str, stream_id: u32) -> Result<()> {
        let mut payload = BytesMut::new();
        amf0::write_publish_command(&mut payload, stream_name, "live");

        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::COMMAND,
                timestamp: 0,
                message_length: payload.len() as u32,
                message_type: MessageType::CommandAMF0,
                message_stream_id: stream_id,
            },
            payload: payload.freeze(),
        };

        write_chunk(stream, &message).await
    }

    /// Read publish response (onStatus)
    async fn read_publish_response(stream: &mut TcpStream, reader: &mut ChunkReader) -> Result<()> {
        for _ in 0..10 {
            let message = reader.read_message(stream).await?;

            if message.header.message_type == MessageType::CommandAMF0 {
                if let Some((cmd, _)) = Self::parse_command_response(&message.payload) {
                    if cmd == "onStatus" {
                        // Check if it's NetStream.Publish.Start
                        if Self::check_publish_start(&message.payload) {
                            return Ok(());
                        }
                    } else if cmd == "_error" {
                        return Err(anyhow!("Publish failed"));
                    }
                }
            }
        }
        Ok(()) // Assume success if no error
    }

    /// Internal send connect command using stateful chunk writer
    async fn send_connect_command_internal(
        connection: Arc<Mutex<RelayConnection>>,
        stream: &mut TcpStream,
        app: &str,
        tc_url: &str,
    ) -> Result<()> {
        let mut payload = BytesMut::new();
        amf0::write_connect_command(&mut payload, app, tc_url);

        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::COMMAND,
                timestamp: 0,
                message_length: payload.len() as u32,
                message_type: MessageType::CommandAMF0,
                message_stream_id: 0,
            },
            payload: payload.freeze(),
        };

        let mut conn = connection.lock().await;
        conn.chunk_writer.write_message(stream, &message).await
    }

    /// Internal send setChunkSize command using stateful chunk writer
    async fn send_set_chunk_size_internal(
        connection: Arc<Mutex<RelayConnection>>,
        stream: &mut TcpStream,
        size: u32,
    ) -> Result<()> {
        let mut payload = BytesMut::with_capacity(4);
        payload.put_u32(size);

        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::PROTOCOL_CONTROL,
                timestamp: 0,
                message_length: 4,
                message_type: MessageType::SetChunkSize,
                message_stream_id: 0,
            },
            payload: payload.freeze(),
        };

        let mut conn = connection.lock().await;
        conn.chunk_writer.write_message(stream, &message).await
    }

    /// Internal send createStream command using stateful chunk writer
    async fn send_create_stream_internal(
        connection: Arc<Mutex<RelayConnection>>,
        stream: &mut TcpStream,
    ) -> Result<()> {
        let mut payload = BytesMut::new();
        amf0::write_create_stream_command(&mut payload);

        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::COMMAND,
                timestamp: 0,
                message_length: payload.len() as u32,
                message_type: MessageType::CommandAMF0,
                message_stream_id: 0,
            },
            payload: payload.freeze(),
        };

        let mut conn = connection.lock().await;
        conn.chunk_writer.write_message(stream, &message).await
    }

    /// Internal send publish command using stateful chunk writer
    async fn send_publish_internal(
        connection: Arc<Mutex<RelayConnection>>,
        stream: &mut TcpStream,
        stream_name: &str,
        stream_id: u32,
    ) -> Result<()> {
        let mut payload = BytesMut::new();
        amf0::write_publish_command(&mut payload, stream_name, "live");

        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::COMMAND,
                timestamp: 0,
                message_length: payload.len() as u32,
                message_type: MessageType::CommandAMF0,
                message_stream_id: stream_id,
            },
            payload: payload.freeze(),
        };

        let mut conn = connection.lock().await;
        conn.chunk_writer.write_message(stream, &message).await
    }

    /// Parse command response to get command name and transaction ID
    fn parse_command_response(payload: &[u8]) -> Option<(String, f64)> {
        if payload.len() < 3 {
            return None;
        }

        let mut cursor = std::io::Cursor::new(payload);

        // Read command name (string)
        let cmd_type = Self::read_u8(&mut cursor).ok()?;
        if cmd_type != amf0::STRING {
            return None;
        }

        let cmd_len = Self::read_u16_be(&mut cursor).ok()?;
        let mut cmd_bytes = vec![0u8; cmd_len as usize];
        if std::io::Read::read_exact(&mut cursor, &mut cmd_bytes).is_err() {
            return None;
        }
        let command = String::from_utf8(cmd_bytes).ok()?;

        // Read transaction ID (number)
        let trans_type = Self::read_u8(&mut cursor).ok()?;
        if trans_type != amf0::NUMBER {
            return None;
        }

        let mut trans_bytes = [0u8; 8];
        if std::io::Read::read_exact(&mut cursor, &mut trans_bytes).is_err() {
            return None;
        }
        let transaction_id = f64::from_be_bytes(trans_bytes);

        Some((command, transaction_id))
    }

    /// Parse stream ID from createStream response
    fn parse_stream_id(payload: &[u8]) -> Option<u32> {
        let mut cursor = std::io::Cursor::new(payload);

        // Skip command name and transaction ID
        let _ = Self::read_u8(&mut cursor); // string type
        let _ = Self::read_u16_be(&mut cursor); // string length
        // Skip string bytes - we'd need to read the length first
        // This is simplified - in real implementation would properly parse AMF
        None
    }

    /// Check if onStatus indicates publish started
    fn check_publish_start(payload: &[u8]) -> bool {
        // Simplified check for "NetStream.Publish.Start" in payload
        let payload_str = String::from_utf8_lossy(payload);
        payload_str.contains("NetStream.Publish.Start")
    }

    /// Send ping (acknowledgment) to keep connection alive
    async fn send_ping(connection: Arc<Mutex<RelayConnection>>) {
        let mut conn = connection.lock().await;
        if !conn.connected {
            return;
        }
        if conn.last_ping.elapsed() < PING_INTERVAL {
            return;
        }

        // Send acknowledgement (protocol control message type 3)
        // In practice, RTMP doesn't require client-side ping, but we track it for reconnection
        conn.last_ping = Instant::now();
    }

    /// Internal disconnect
    async fn disconnect_internal(connection: Arc<Mutex<RelayConnection>>) {
        let mut conn = connection.lock().await;
        if let Some(mut stream) = conn.stream.take() {
            let _ = stream.shutdown().await;
        }
        conn.connected = false;
        conn.connecting = false;
    }

    /// Set chunk size internally
    async fn set_chunk_size_internal(connection: Arc<Mutex<RelayConnection>>, size: u32) {
        let mut conn = connection.lock().await;
        if conn.connected {
            conn.chunk_size = size;
            conn.chunk_writer.set_chunk_size(size);
        }
    }

    /// Internal send message
    async fn send_message_internal(connection: Arc<Mutex<RelayConnection>>, message: RtmpMessage) {
        // Prepare the message with correct stream ID and chunk stream ID
        let mut msg = message;
        {
            let mut conn = connection.lock().await;
            if !conn.connected {
                return;
            }
            let Some(_s) = conn.stream.as_mut() else {
                return;
            };
            // Update message stream ID for video/audio/data
            match msg.header.message_type {
                MessageType::Video | MessageType::Audio | MessageType::Metadata => {
                    msg.header.message_stream_id = conn.stream_id;
                    msg.header.chunk_stream_id = match msg.header.message_type {
                        MessageType::Video => csid::VIDEO,
                        MessageType::Audio => csid::AUDIO,
                        MessageType::Metadata => csid::DATA,
                        _ => csid::DATA,
                    };
                }
                _ => {}
            }
        }

        // Now get the stream and send using stateful chunk_writer
        let mut stream = {
            let mut conn = connection.lock().await;
            conn.stream.take()
        };

        let Some(mut stream) = stream else {
            return;
        };

        // Use the stateful chunk_writer to maintain per-CSID state across messages
        let write_result = {
            let mut conn = connection.lock().await;
            conn.chunk_writer.write_message(&mut stream, &msg).await
        };

        match write_result {
            Ok(()) => {
                let mut conn = connection.lock().await;
                conn.bytes_sent += msg.payload.len() as u64;
                conn.stream = Some(stream);
            }
            Err(error) => {
                let mut conn = connection.lock().await;
                conn.last_error = Some(error.to_string());
                conn.connected = false;
                conn.stream = None;
                drop(conn);
                // Trigger reconnect
                Self::schedule_reconnect(connection).await;
            }
        }
    }

    /// Internal update output
    async fn update_output_internal(connection: Arc<Mutex<RelayConnection>>, output: PlatformOutput) {
        let mut conn = connection.lock().await;
        let was_enabled = conn.output.enabled;
        conn.output = output.clone();

        if !was_enabled && output.enabled {
            // Need to reconnect
            conn.connected = false;
            conn.connecting = false;
            drop(conn);
            Self::connect_internal(connection.clone()).await;
        } else if was_enabled && !output.enabled {
            // Disconnect
            let _ = conn.message_tx.send(RelayMessage::Disconnect).await;
        }
    }

    /// Schedule reconnection with exponential backoff
    async fn schedule_reconnect(connection: Arc<Mutex<RelayConnection>>) {
        let (attempts, enabled) = {
            let mut conn = connection.lock().await;
            conn.connected = false;
            conn.connecting = false;
            conn.stream = None;
            conn.reconnect_attempts += 1;
            let attempts = conn.reconnect_attempts;
            let enabled = conn.output.enabled;
            (attempts, enabled)
        };

        if !enabled || attempts > MAX_RECONNECT_ATTEMPTS {
            if attempts > MAX_RECONNECT_ATTEMPTS {
                let mut conn = connection.lock().await;
                conn.last_error = Some("Max reconnection attempts reached".to_string());
                error!("Relay max reconnection attempts reached");
            }
            return;
        }

        // Exponential backoff with jitter
        let base_delay = INITIAL_RECONNECT_DELAY.as_millis() as u64;
        let delay_ms = std::cmp::min(
            base_delay * (2_u64.pow(attempts.saturating_sub(1))),
            MAX_RECONNECT_DELAY.as_millis() as u64,
        );
        // Add jitter (±25%) using CSPRNG
        let jitter = (delay_ms as f64 * 0.25 * (OsRng.next_u64() as f64 / u64::MAX as f64 * 2.0 - 1.0)) as u64;
        let final_delay = Duration::from_millis(delay_ms.saturating_add(jitter).max(100));

        warn!("Relay scheduling reconnect in {:?} (attempt {}/{})", final_delay, attempts, MAX_RECONNECT_ATTEMPTS);

        sleep(final_delay).await;

        // Check if still enabled and not connected
        let should_reconnect = {
            let conn = connection.lock().await;
            conn.output.enabled && !conn.connected && !conn.connecting
        };

        if should_reconnect {
            let _ = connection.lock().await.message_tx.send(RelayMessage::Connect).await;
        }
    }

    /// Parse RTMP URL with SSRF protection
    fn parse_rtmp_url(url: &str, stream_key: &str) -> Result<(String, u16, String, String)> {
        // rtmp://host:port/app/stream
        if url.starts_with("rtmps://") {
            anyhow::bail!("RTMPS is not supported yet; use an rtmp:// endpoint");
        }
        let url = url.strip_prefix("rtmp://").unwrap_or(url);

        // Reject URLs with embedded credentials (user:pass@host)
        if url.contains('@') {
            anyhow::bail!("URL contains embedded credentials, not allowed");
        }

        let parts: Vec<&str> = url.split('/').collect();
        if parts.len() < 3 {
            anyhow::bail!("Invalid RTMP URL format");
        }

        let host_port = parts[0];
        let app = parts[1].to_string();
        let stream = if parts.len() > 2 {
            parts[2..].join("/")
        } else {
            stream_key.to_string()
        };

        let (host, port) = if let Some(idx) = host_port.rfind(':') {
            let host = &host_port[..idx];
            let port_str = &host_port[idx+1..];

            // Validate port range
            let port = port_str.parse::<u16>().map_err(|_| anyhow::anyhow!("Invalid port number"))?;
            if port == 0 {
                anyhow::bail!("Port 0 is not allowed");
            }
            (host.to_string(), port)
        } else {
            (host_port.to_string(), 1935)
        };

        // SSRF Protection: Block private/internal IP ranges.
        // Local testing against an on-machine RTMP sink is allowed only in
        // debug builds when explicitly opted in.
        if !crate::rtmp::allow_private_relay_targets() && Self::is_private_or_internal_host(&host) {
            anyhow::bail!("Connection to private/internal addresses is not allowed: {}", host);
        }

        // Validate app name (alphanumeric + safe chars only)
        if !Self::is_valid_app_name(&app) {
            anyhow::bail!("Invalid app name: contains unsafe characters");
        }

        // Validate stream name (alphanumeric + safe chars only)
        if !Self::is_valid_stream_name(&stream) {
            anyhow::bail!("Invalid stream name: contains unsafe characters");
        }

        Ok((host, port, app, stream))
    }

    /// Check if host is a private/internal address that should be blocked
    fn is_private_or_internal_host(host: &str) -> bool {
        // Check for localhost variants
        let host_lower = host.to_lowercase();
        if host_lower == "localhost" || host_lower == "localhost.localdomain" {
            return true;
        }

        // Try to parse as IP address
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(ipv4) => {
                    let octets = ipv4.octets();
                    // 10.0.0.0/8
                    if octets[0] == 10 { return true; }
                    // 172.16.0.0/12
                    if octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31 { return true; }
                    // 192.168.0.0/16
                    if octets[0] == 192 && octets[1] == 168 { return true; }
                    // 127.0.0.0/8 (loopback)
                    if octets[0] == 127 { return true; }
                    // 169.254.0.0/16 (link-local)
                    if octets[0] == 169 && octets[1] == 254 { return true; }
                    // 0.0.0.0 (unspecified)
                    if octets[0] == 0 && octets[1] == 0 && octets[2] == 0 && octets[3] == 0 { return true; }
                    // 224.0.0.0/4 (multicast)
                    if octets[0] >= 224 { return true; }
                    // 240.0.0.0/4 (reserved)
                    if octets[0] >= 240 { return true; }
                    // 255.255.255.255 (broadcast)
                    if octets == [255, 255, 255, 255] { return true; }
                }
                std::net::IpAddr::V6(ipv6) => {
                    // ::1 (loopback)
                    if ipv6 == std::net::Ipv6Addr::LOCALHOST { return true; }
                    // :: (unspecified)
                    if ipv6 == std::net::Ipv6Addr::UNSPECIFIED { return true; }
                    // fe80::/10 (link-local)
                    if ipv6.segments()[0] & 0xffc0 == 0xfe80 { return true; }
                    // fc00::/7 (unique local)
                    if ipv6.segments()[0] & 0xfe00 == 0xfc00 { return true; }
                    // ff00::/8 (multicast)
                    if ipv6.segments()[0] & 0xff00 == 0xff00 { return true; }
                }
            }
        }

        false
    }

    /// Validate app name contains only safe characters
    fn is_valid_app_name(app: &str) -> bool {
        // Allow alphanumeric, hyphen, underscore, dot
        app.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    }

    /// Validate stream name contains only safe characters
    fn is_valid_stream_name(stream: &str) -> bool {
        // Allow alphanumeric, hyphen, underscore, dot, slash (for nested streams)
        stream.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
    }

    // AMF0 read helpers
    fn read_u8(cursor: &mut std::io::Cursor<&[u8]>) -> Result<u8> {
        let mut b = [0u8; 1];
        std::io::Read::read_exact(cursor, &mut b)?;
        Ok(b[0])
    }

    fn read_u16_be(cursor: &mut std::io::Cursor<&[u8]>) -> Result<u16> {
        let mut b = [0u8; 2];
        std::io::Read::read_exact(cursor, &mut b)?;
        Ok(u16::from_be_bytes(b))
    }
}

impl Default for RelayManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rtmp_url() {
        let (host, port, app, stream) = RelayManager::parse_rtmp_url(
            "rtmp://live.twitch.tv/app/live_12345_abcdef",
            "fallback_key"
        ).unwrap();
        assert_eq!(host, "live.twitch.tv");
        assert_eq!(port, 1935);
        assert_eq!(app, "app");
        assert_eq!(stream, "live_12345_abcdef");

        let (host, port, app, stream) = RelayManager::parse_rtmp_url(
            "rtmp://custom.server:1936/live/stream_key",
            "fallback"
        ).unwrap();
        assert_eq!(host, "custom.server");
        assert_eq!(port, 1936);
        assert_eq!(app, "live");
        assert_eq!(stream, "stream_key");
    }
}