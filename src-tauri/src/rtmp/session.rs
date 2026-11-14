//! Session management for RTMP connections

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex, mpsc};
use tracing::{info, instrument};
use bytes::{Bytes, BytesMut};
use anyhow::Result;

use crate::rtmp::buffer::{RingBuffer, BufferedFrame, FrameType};
use crate::rtmp::protocol::{RtmpMessage, MessageType, ChunkStreamId};
use crate::ipc::DelayMode;

/// Session configuration
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub default_delay_sec: u32,
    pub delay_mode: DelayMode,
    pub buffer_config: crate::rtmp::BufferConfig,
}

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SessionState {
    Handshake,
    Connecting,
    Streaming,
    Paused,
    Closed,
}

/// RTMP Session
pub struct Session {
    pub addr: SocketAddr,
    pub state: Arc<RwLock<SessionState>>,
    pub config: Arc<RwLock<SessionConfig>>,
    pub buffer: Arc<RingBuffer>,
    pub stream_id: Option<String>,
    pub app_name: Option<String>,
    pub created_at: Instant,
    pub last_activity: Arc<RwLock<Instant>>,
    pub bytes_received: Arc<RwLock<u64>>,
    pub bytes_sent: Arc<RwLock<u64>>,
    pub video_frame_count: Arc<RwLock<u64>>,
    pub audio_frame_count: Arc<RwLock<u64>>,
    pub chunk_streams: Arc<Mutex<HashMap<ChunkStreamId, ChunkStreamState>>>,
    pub event_tx: mpsc::Sender<SessionEvent>,
}

/// Chunk stream reassembly state
#[derive(Debug, Default)]
pub struct ChunkStreamState {
    pub header: Option<crate::rtmp::protocol::ChunkHeader>,
    pub buffer: BytesMut,
    pub expected_size: usize,
    pub received_size: usize,
}

/// Session events for external handling
#[derive(Debug, Clone)]
pub enum SessionEvent {
    StreamStarted { stream_key: String },
    StreamEnded,
    VideoKeyframe { timestamp: u32 },
    VideoFrame { timestamp: u32 },
    AudioFrame { timestamp: u32 },
    Metadata { data: Bytes },
    Error { message: String },
}

/// Incoming stream info
#[derive(Debug, Clone)]
pub struct IncomingStream {
    pub stream_key: String,
    pub app_name: String,
    pub addr: SocketAddr,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<(u32, u32)>,
    pub fps: Option<f32>,
    pub bitrate: Option<u32>,
}

/// Session manager
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SocketAddr, Arc<Session>>>>,
    buffer: Arc<RingBuffer>,
    event_tx: mpsc::Sender<SessionEvent>,
    event_rx: Arc<Mutex<mpsc::Receiver<SessionEvent>>>,
    delay_sec: Arc<RwLock<u32>>,
    delay_mode: Arc<RwLock<DelayMode>>,
}

impl SessionManager {
    /// Create new session manager
    pub fn new(buffer: Arc<RingBuffer>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            buffer,
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            delay_sec: Arc::new(RwLock::new(10)),
            delay_mode: Arc::new(RwLock::new(DelayMode::Instant)),
        }
    }

    /// Create new session for incoming connection
    #[instrument(skip(self, buffer))]
    pub async fn create_session(
        &self,
        addr: SocketAddr,
        config: SessionConfig,
        buffer: Arc<RingBuffer>,
    ) -> Result<Arc<Session>> {
        let (event_tx, _) = mpsc::channel(100);
        let session = Arc::new(Session {
            addr,
            state: Arc::new(RwLock::new(SessionState::Handshake)),
            config: Arc::new(RwLock::new(config)),
            buffer,
            stream_id: None,
            app_name: None,
            created_at: Instant::now(),
            last_activity: Arc::new(RwLock::new(Instant::now())),
            bytes_received: Arc::new(RwLock::new(0)),
            bytes_sent: Arc::new(RwLock::new(0)),
            video_frame_count: Arc::new(RwLock::new(0)),
            audio_frame_count: Arc::new(RwLock::new(0)),
            chunk_streams: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        });

        self.sessions.write().await.insert(addr, session.clone());
        info!("Created session for {}", addr);
        Ok(session)
    }

    /// Remove session
    pub async fn remove_session(&self, addr: &SocketAddr) {
        self.sessions.write().await.remove(addr);
        info!("Removed session for {}", addr);
    }

    /// Get session by address
    pub async fn get_session(&self, addr: &SocketAddr) -> Option<Arc<Session>> {
        self.sessions.read().await.get(addr).cloned()
    }

    /// Get all active sessions
    pub async fn get_active_sessions(&self) -> Vec<Arc<Session>> {
        self.sessions.read().await.values().cloned().collect()
    }

    /// Close all sessions
    pub async fn close_all(&self) {
        let sessions = self.sessions.read().await.clone();
        for (addr, session) in sessions {
            *session.state.write().await = SessionState::Closed;
        }
        info!("Closed all sessions");
    }

    /// Update delay for all sessions
    pub async fn update_delay(&self, delay_sec: u32) {
        *self.delay_sec.write().await = delay_sec;
        let sessions = self.sessions.read().await;
        for session in sessions.values() {
            session.config.write().await.default_delay_sec = delay_sec;
            session.buffer.set_delay(delay_sec).await;
        }
    }

    /// Set delay mode for all sessions
    pub async fn set_delay_mode(&self, mode: DelayMode) {
        *self.delay_mode.write().await = mode.clone();
        let sessions = self.sessions.read().await;
        for session in sessions.values() {
            session.config.write().await.delay_mode = mode;
        }
    }

    /// Skip to live for all sessions
    pub async fn skip_to_live(&self) {
        self.buffer.skip_to_live().await;
        let sessions = self.sessions.read().await;
        for session in sessions.values() {
            // Reset session state to start fresh
            *session.state.write().await = SessionState::Streaming;
        }
    }

    /// Get event receiver for external processing
    pub fn event_receiver(&self) -> Arc<Mutex<mpsc::Receiver<SessionEvent>>> {
        self.event_rx.clone()
    }
}

impl Session {
    /// Process incoming RTMP message
    #[instrument(skip(self, message))]
    pub async fn process_message(&self, message: RtmpMessage) -> Result<Option<RtmpMessage>> {
        // Update activity
        *self.last_activity.write().await = Instant::now();
        *self.bytes_received.write().await += message.payload.len() as u64;

        // Extract timestamp and frame info
        let timestamp = message.header.timestamp;
        let is_keyframe = self.is_video_keyframe(&message);
        let frame_type = match message.header.message_type {
            MessageType::Video => FrameType::Video,
            MessageType::Audio => FrameType::Audio,
            MessageType::Metadata => FrameType::Metadata,
            _ => FrameType::Unknown,
        };

        // Update frame counters
        match frame_type {
            FrameType::Video => {
                *self.video_frame_count.write().await += 1;
                if is_keyframe {
                    let _ = self.event_tx.try_send(SessionEvent::VideoKeyframe { timestamp });
                }
                let _ = self.event_tx.try_send(SessionEvent::VideoFrame { timestamp });
            }
            FrameType::Audio => {
                *self.audio_frame_count.write().await += 1;
                let _ = self.event_tx.try_send(SessionEvent::AudioFrame { timestamp });
            }
            FrameType::Metadata => {
                let _ = self.event_tx.try_send(SessionEvent::Metadata { data: message.payload.clone() });
            }
            _ => {}
        }

        // Check delay mode
        let config = self.config.read().await;
        let delay_mode = config.delay_mode.clone();
        let delay_sec = config.default_delay_sec;
        drop(config);

        // In disabled mode, pass through immediately
        if delay_mode == DelayMode::Disabled || delay_sec == 0 {
            return Ok(Some(message));
        }

        // Create buffered frame
        let buffered = BufferedFrame {
            data: message.payload,
            timestamp,
            timestamp_ns: current_timestamp_ns(),
            frame_type,
            is_keyframe,
            stream_id: message.header.chunk_stream_id as u32,
        };

        // Push to buffer
        self.buffer.push(buffered).await?;

        // In Instant mode, return ready frames immediately
        if delay_mode == DelayMode::Instant {
            let ready = self.buffer.pop_ready(current_timestamp_ns()).await;
            if let Some(frame) = ready.first() {
                // Reconstruct message from buffered frame
                return Ok(Some(RtmpMessage {
                    header: crate::rtmp::protocol::ChunkHeader {
                        chunk_stream_id: frame.stream_id as u32,
                        timestamp: frame.timestamp,
                        message_length: frame.data.len() as u32,
                        message_type: match frame.frame_type {
                            FrameType::Video => MessageType::Video,
                            FrameType::Audio => MessageType::Audio,
                            FrameType::Metadata => MessageType::Metadata,
                            _ => MessageType::Unknown,
                        },
                        message_stream_id: 0,
                    },
                    payload: frame.data.clone(),
                }));
            }
        }

        // In Normal mode, don't return frames yet (buffering)
        Ok(None)
    }

    /// Check if video frame is a keyframe (simplified)
    fn is_video_keyframe(&self, message: &RtmpMessage) -> bool {
        if message.header.message_type != MessageType::Video {
            return false;
        }
        if message.payload.len() < 5 {
            return false;
        }
        // Check frame type in H.264/AVC or H.265/HEVC
        let frame_type = message.payload[0] >> 4;
        frame_type == 1 // Keyframe
    }

    /// Get session info
    pub async fn get_info(&self) -> SessionInfo {
        SessionInfo {
            addr: self.addr,
            state: *self.state.read().await,
            stream_id: self.stream_id.clone(),
            app_name: self.app_name.clone(),
            uptime_sec: self.created_at.elapsed().as_secs(),
            bytes_received: *self.bytes_received.read().await,
            bytes_sent: *self.bytes_sent.read().await,
            video_frames: *self.video_frame_count.read().await,
            audio_frames: *self.audio_frame_count.read().await,
            is_buffering: self.buffer.is_buffering().await,
        }
    }
}

/// Session information for UI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionInfo {
    pub addr: SocketAddr,
    pub state: SessionState,
    pub stream_id: Option<String>,
    pub app_name: Option<String>,
    pub uptime_sec: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub video_frames: u64,
    pub audio_frames: u64,
    pub is_buffering: bool,
}

/// Utility function for current timestamp in nanoseconds
pub fn current_timestamp_ns() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn updates_existing_session_without_relocking_config() {
        let buffer = Arc::new(RingBuffer::new(crate::rtmp::BufferConfig::default()));
        let manager = SessionManager::new(buffer.clone());
        let session = manager.create_session(
            "127.0.0.1:12345".parse().unwrap(),
            SessionConfig {
                default_delay_sec: 10,
                delay_mode: DelayMode::Instant,
                buffer_config: crate::rtmp::BufferConfig::default(),
            },
            buffer,
        ).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            manager.update_delay(20).await;
            manager.set_delay_mode(DelayMode::Normal).await;
        }).await.expect("session configuration update deadlocked");
        let config = session.config.read().await;
        assert_eq!(config.default_delay_sec, 20);
        assert_eq!(config.delay_mode, DelayMode::Normal);
    }
}