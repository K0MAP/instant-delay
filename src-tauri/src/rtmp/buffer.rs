//! Ring Buffer for video/audio frame storage with delay support

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tracing::instrument;
use serde::{Deserialize, Serialize};
use bytes::Bytes;

/// Buffer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferConfig {
    pub max_delay_sec: u32,
    pub max_video_frames: usize,
    pub max_audio_frames: usize,
    pub max_metadata_frames: usize,
    pub max_memory_mb: usize,
    pub keyframe_only: bool,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            max_delay_sec: 600,
            max_video_frames: 18000,   // 300 sec * 60 fps
            max_audio_frames: 30000,   // 300 sec * 100 fps (audio packets)
            max_metadata_frames: 100,
            max_memory_mb: 512,
            keyframe_only: false,
        }
    }
}

/// Buffered frame
#[derive(Debug, Clone)]
pub struct BufferedFrame {
    pub data: Bytes,
    pub timestamp: u32,           // RTMP timestamp (ms)
    pub timestamp_ns: u64,        // Wall clock timestamp (ns)
    pub frame_type: FrameType,
    pub is_keyframe: bool,
    pub stream_id: u32,
}

/// Frame types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameType {
    Video,
    Audio,
    Metadata,
    Unknown,
}

/// Buffer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferStats {
    pub video_frames: usize,
    pub audio_frames: usize,
    pub metadata_frames: usize,
    pub total_frames: usize,
    pub memory_usage_bytes: usize,
    pub memory_usage_mb: f64,
    pub oldest_timestamp: Option<u32>,
    pub newest_timestamp: Option<u32>,
    pub delay_span_sec: f64,
    pub fill_percent: f64,
    pub is_buffering: bool,
    pub target_delay_sec: u32,
}

/// Ring buffer for delayed streaming
pub struct RingBuffer {
    config: Arc<RwLock<BufferConfig>>,
    video_frames: Arc<Mutex<VecDeque<BufferedFrame>>>,
    audio_frames: Arc<Mutex<VecDeque<BufferedFrame>>>,
    metadata_frames: Arc<Mutex<VecDeque<BufferedFrame>>>,
    current_delay_sec: Arc<RwLock<u32>>,
    is_buffering: Arc<RwLock<bool>>,
    last_keyframe_time: Arc<RwLock<Option<Instant>>>,
    stats_cache: Arc<RwLock<BufferStats>>,
}

impl RingBuffer {
    /// Create new ring buffer
    pub fn new(config: BufferConfig) -> Self {
        let config = Arc::new(RwLock::new(config));
        Self {
            config: config.clone(),
            video_frames: Arc::new(Mutex::new(VecDeque::new())),
            audio_frames: Arc::new(Mutex::new(VecDeque::new())),
            metadata_frames: Arc::new(Mutex::new(VecDeque::new())),
            current_delay_sec: Arc::new(RwLock::new(10)),
            is_buffering: Arc::new(RwLock::new(true)),
            last_keyframe_time: Arc::new(RwLock::new(None)),
            stats_cache: Arc::new(RwLock::new(BufferStats::default())),
        }
    }

    /// Push a frame into the buffer
    #[instrument(skip(self, frame))]
    pub async fn push(&self, frame: BufferedFrame) -> Result<(), BufferError> {
        let config = self.config.read().await;
        let delay_sec = *self.current_delay_sec.read().await;
        drop(config);

        match frame.frame_type {
            FrameType::Video => {
                let mut frames = self.video_frames.lock().await;
                self.enforce_capacity(&mut frames, delay_sec, FrameType::Video).await;
                frames.push_back(frame);
            }
            FrameType::Audio => {
                let mut frames = self.audio_frames.lock().await;
                self.enforce_capacity(&mut frames, delay_sec, FrameType::Audio).await;
                frames.push_back(frame);
            }
            FrameType::Metadata => {
                let mut frames = self.metadata_frames.lock().await;
                if frames.len() >= self.config.read().await.max_metadata_frames {
                    frames.pop_front();
                }
                frames.push_back(frame);
            }
            FrameType::Unknown => {}
        }

        // Update buffering state
        self.update_buffering_state().await;
        self.update_stats().await;

        Ok(())
    }

    /// Pop frames that are ready for output (older than delay)
    #[instrument(skip(self))]
    pub async fn pop_ready(&self, now_ns: u64) -> Vec<BufferedFrame> {
        let delay_ns = (*self.current_delay_sec.read().await as u64) * 1_000_000_000;
        let mut ready = Vec::new();

        // Pop video frames
        {
            let mut frames = self.video_frames.lock().await;
            while let Some(frame) = frames.front() {
                let age_ns = now_ns.saturating_sub(frame.timestamp_ns);
                if age_ns >= delay_ns {
                    ready.push(frames.pop_front().unwrap());
                } else {
                    break;
                }
            }
        }

        // Pop audio frames
        {
            let mut frames = self.audio_frames.lock().await;
            while let Some(frame) = frames.front() {
                let age_ns = now_ns.saturating_sub(frame.timestamp_ns);
                if age_ns >= delay_ns {
                    ready.push(frames.pop_front().unwrap());
                } else {
                    break;
                }
            }
        }

        ready
    }

    /// Pop frames that are ready for output, merged across media queues in
    /// arrival order. Video and audio must not be drained independently or
    /// their relative order is lost.
    pub async fn pop_ready_ordered(&self, now_ns: u64, max_frames: usize) -> Vec<BufferedFrame> {
        let delay_ns = (*self.current_delay_sec.read().await as u64) * 1_000_000_000;
        let mut ready = Vec::new();

        let mut video = self.video_frames.lock().await;
        let mut audio = self.audio_frames.lock().await;
        let mut metadata = self.metadata_frames.lock().await;

        while ready.len() < max_frames {
            let candidates = [
                video.front().map(|f| (0u8, f.timestamp_ns)),
                audio.front().map(|f| (1u8, f.timestamp_ns)),
                metadata.front().map(|f| (2u8, f.timestamp_ns)),
            ];
            let Some((queue, timestamp_ns)) = candidates
                .into_iter()
                .flatten()
                .min_by_key(|(_, ts)| *ts)
            else {
                break;
            };

            if now_ns.saturating_sub(timestamp_ns) < delay_ns {
                break;
            }

            let frame = match queue {
                0 => video.pop_front(),
                1 => audio.pop_front(),
                _ => metadata.pop_front(),
            };
            match frame {
                Some(frame) => ready.push(frame),
                None => break,
            }
        }

        ready
    }

    /// Get frames ready for output (non-blocking, for relay)
    #[instrument(skip(self))]
    pub async fn get_ready_frames(&self, now_ns: u64, max_frames: usize) -> Vec<BufferedFrame> {
        let delay_ns = (*self.current_delay_sec.read().await as u64) * 1_000_000_000;
        let mut ready = Vec::new();

        // Video frames
        {
            let mut frames = self.video_frames.lock().await;
            while ready.len() < max_frames {
                if let Some(frame) = frames.front() {
                    let age_ns = now_ns.saturating_sub(frame.timestamp_ns);
                    if age_ns >= delay_ns {
                        ready.push(frames.pop_front().unwrap());
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        // Audio frames
        {
            let mut frames = self.audio_frames.lock().await;
            while ready.len() < max_frames {
                if let Some(frame) = frames.front() {
                    let age_ns = now_ns.saturating_sub(frame.timestamp_ns);
                    if age_ns >= delay_ns {
                        ready.push(frames.pop_front().unwrap());
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        ready
    }

    /// Enforce buffer capacity limits (both frame count and memory)
    async fn enforce_capacity(
        &self,
        frames: &mut VecDeque<BufferedFrame>,
        delay_sec: u32,
        frame_type: FrameType,
    ) {
        let config = self.config.read().await;
        let max_frames = match frame_type {
            FrameType::Video => config.max_video_frames,
            FrameType::Audio => config.max_audio_frames,
            FrameType::Metadata => config.max_metadata_frames,
            FrameType::Unknown => 100,
        };
        let max_memory_mb = config.max_memory_mb;
        let max_memory_bytes = max_memory_mb.saturating_mul(1024 * 1024);
        drop(config);

        // Remove oldest frames if over frame capacity
        while frames.len() >= max_frames {
            frames.pop_front();
        }

        // Enforce memory limit - remove oldest frames until under limit
        let current_memory: usize = frames.iter().map(|f| f.data.len()).sum();
        if current_memory > max_memory_bytes {
            let mut removed = 0;
            while !frames.is_empty() {
                let current_memory: usize = frames.iter().map(|f| f.data.len()).sum();
                if current_memory <= max_memory_bytes {
                    break;
                }
                frames.pop_front();
                removed += 1;
            }
            tracing::warn!("RingBuffer: dropped {} frames to enforce memory limit ({} MB)", removed, max_memory_mb);
        }
    }

    /// Update buffering state (true if buffer not yet full enough for delay)
    async fn update_buffering_state(&self) {
        let delay_sec = *self.current_delay_sec.read().await;
        if delay_sec == 0 {
            *self.is_buffering.write().await = false;
            return;
        }

        let video_frames = self.video_frames.lock().await;
        let has_enough_video = !video_frames.is_empty();
        drop(video_frames);

        // Check if we have at least delay_sec worth of frames
        let video_frames = self.video_frames.lock().await;
        let oldest = video_frames.front().map(|f| f.timestamp_ns);
        let newest = video_frames.back().map(|f| f.timestamp_ns);
        drop(video_frames);

        let is_buffering = if let (Some(oldest), Some(newest)) = (oldest, newest) {
            let span_ns = newest.saturating_sub(oldest);
            span_ns < (delay_sec as u64 * 1_000_000_000)
        } else {
            true
        };

        *self.is_buffering.write().await = is_buffering;
    }

    /// Update cached statistics
    async fn update_stats(&self) {
        let video_frames = self.video_frames.lock().await;
        let audio_frames = self.audio_frames.lock().await;
        let metadata_frames = self.metadata_frames.lock().await;

        let v_count = video_frames.len();
        let a_count = audio_frames.len();
        let m_count = metadata_frames.len();

        let memory_usage: usize = video_frames.iter().map(|f| f.data.len()).sum();
        let memory_usage = memory_usage + audio_frames.iter().map(|f| f.data.len()).sum::<usize>();

        let oldest_ts = video_frames.front().map(|f| f.timestamp);
        let newest_ts = video_frames.back().map(|f| f.timestamp);

        let delay_span = if let (Some(oldest), Some(newest)) = (oldest_ts, newest_ts) {
            (newest.saturating_sub(oldest)) as f64 / 1000.0
        } else {
            0.0
        };

        let config = self.config.read().await;
        let target_delay = *self.current_delay_sec.read().await;
        let max_frames = config.max_video_frames + config.max_audio_frames;
        let fill_percent = if max_frames > 0 {
            ((v_count + a_count) as f64 / max_frames as f64) * 100.0
        } else { 0.0 };
        drop(config);

        let mut stats = self.stats_cache.write().await;
        stats.video_frames = v_count;
        stats.audio_frames = a_count;
        stats.metadata_frames = m_count;
        stats.total_frames = v_count + a_count + m_count;
        stats.memory_usage_bytes = memory_usage;
        stats.memory_usage_mb = memory_usage as f64 / (1024.0 * 1024.0);
        stats.oldest_timestamp = oldest_ts;
        stats.newest_timestamp = newest_ts;
        stats.delay_span_sec = delay_span;
        stats.fill_percent = fill_percent.min(100.0);
        stats.is_buffering = *self.is_buffering.read().await;
        stats.target_delay_sec = target_delay;
    }

    /// Get current buffer statistics
    pub async fn get_stats(&self) -> BufferStats {
        self.stats_cache.read().await.clone()
    }

    /// Set target delay
    pub async fn set_delay(&self, delay_sec: u32) {
        let config = self.config.read().await;
        let clamped = delay_sec.min(config.max_delay_sec);
        drop(config);
        *self.current_delay_sec.write().await = clamped;
        self.update_buffering_state().await;
    }

    /// Get current delay
    pub async fn get_delay(&self) -> u32 {
        *self.current_delay_sec.read().await
    }

    /// Check if currently buffering
    pub async fn is_buffering(&self) -> bool {
        *self.is_buffering.read().await
    }

    /// Clear all buffers
    pub async fn clear(&self) {
        self.video_frames.lock().await.clear();
        self.audio_frames.lock().await.clear();
        self.metadata_frames.lock().await.clear();
        *self.is_buffering.write().await = true;
        self.update_stats().await;
    }

    /// Skip to live (clear buffer but keep new frames)
    pub async fn skip_to_live(&self) {
        self.clear().await;
        *self.is_buffering.write().await = false;
    }

    /// Get buffer fill percentage (0.0 - 1.0)
    pub async fn fill_ratio(&self) -> f64 {
        let stats = self.get_stats().await;
        stats.fill_percent / 100.0
    }

    /// Get estimated time until buffer is full
    pub async fn time_until_ready(&self) -> Option<Duration> {
        let stats = self.get_stats().await;
        if !stats.is_buffering || stats.delay_span_sec <= 0.0 {
            return None;
        }
        let remaining = stats.target_delay_sec as f64 - stats.delay_span_sec;
        if remaining <= 0.0 {
            return None;
        }
        Some(Duration::from_secs_f64(remaining))
    }
}

/// Buffer errors
#[derive(Debug, thiserror::Error)]
pub enum BufferError {
    #[error("Buffer full")]
    Full,
    #[error("Invalid frame")]
    InvalidFrame,
    #[error("Configuration error: {0}")]
    Config(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn clamps_requested_delay_to_buffer_configuration() {
        let buffer = RingBuffer::new(BufferConfig { max_delay_sec: 30, ..Default::default() });
        buffer.set_delay(100).await;
        assert_eq!(buffer.get_delay().await, 30);
        buffer.set_delay(0).await;
        assert!(!buffer.is_buffering().await);
    }

    #[tokio::test]
    async fn releases_audio_only_after_requested_delay() {
        let buffer = RingBuffer::new(BufferConfig::default());
        buffer.set_delay(1).await;
        buffer.push(BufferedFrame {
            data: Bytes::from_static(b"audio"),
            timestamp: 0,
            timestamp_ns: 1_000_000_000,
            frame_type: FrameType::Audio,
            is_keyframe: false,
            stream_id: 4,
        }).await.unwrap();
        assert!(buffer.pop_ready(1_999_999_999).await.is_empty());
        let frames = buffer.pop_ready(2_000_000_000).await;
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, Bytes::from_static(b"audio"));
        assert!(buffer.pop_ready(3_000_000_000).await.is_empty());
    }
}

impl Default for BufferStats {
    fn default() -> Self {
        Self {
            video_frames: 0,
            audio_frames: 0,
            metadata_frames: 0,
            total_frames: 0,
            memory_usage_bytes: 0,
            memory_usage_mb: 0.0,
            oldest_timestamp: None,
            newest_timestamp: None,
            delay_span_sec: 0.0,
            fill_percent: 0.0,
            is_buffering: true,
            target_delay_sec: 10,
        }
    }
}