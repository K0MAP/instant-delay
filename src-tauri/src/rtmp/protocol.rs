//! RTMP Protocol implementation
//!
//! The reader is stateful: RTMP messages may span several chunks and the
//! header of a chunk may only carry the fields that changed since the previous
//! chunk on the same chunk stream. A single [`ChunkReader`] must therefore be
//! reused for the lifetime of a connection.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use bytes::{Bytes, BytesMut, BufMut};
use tracing::debug;

pub type ChunkStreamId = u32;
use anyhow::{anyhow, bail, Result};

/// Default chunk size mandated by the RTMP specification.
pub const DEFAULT_CHUNK_SIZE: u32 = 128;

/// Largest chunk size we are willing to accept from a peer (16 MiB).
pub const MAX_CHUNK_SIZE: u32 = 16 * 1024 * 1024;

/// Largest single RTMP message we are willing to buffer (16 MiB).
pub const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// RTMP Chunk Header
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkHeader {
    pub chunk_stream_id: u32,
    pub timestamp: u32,
    pub message_length: u32,
    pub message_type: MessageType,
    pub message_stream_id: u32,
}

/// RTMP Message types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    SetChunkSize = 1,
    Abort = 2,
    Acknowledgement = 3,
    UserControl = 4,
    WindowAcknowledgementSize = 5,
    SetPeerBandwidth = 6,
    Audio = 8,
    Video = 9,
    MetadataAMF3 = 15,
    Metadata = 18, // AMF0 data
    CommandAMF3 = 17,
    SharedObjectAMF3 = 16,
    SharedObjectAMF0 = 19,
    CommandAMF0 = 20,
    Aggregate = 22,
    Unknown = 255,
}

impl Default for MessageType {
    fn default() -> Self {
        MessageType::Unknown
    }
}

impl From<u8> for MessageType {
    fn from(v: u8) -> Self {
        match v {
            1 => MessageType::SetChunkSize,
            2 => MessageType::Abort,
            3 => MessageType::Acknowledgement,
            4 => MessageType::UserControl,
            5 => MessageType::WindowAcknowledgementSize,
            6 => MessageType::SetPeerBandwidth,
            8 => MessageType::Audio,
            9 => MessageType::Video,
            16 => MessageType::SharedObjectAMF3,
            17 => MessageType::CommandAMF3,
            18 => MessageType::Metadata,
            19 => MessageType::SharedObjectAMF0,
            20 => MessageType::CommandAMF0,
            22 => MessageType::Aggregate,
            _ => MessageType::Unknown,
        }
    }
}

/// RTMP Message
#[derive(Debug, Clone)]
pub struct RtmpMessage {
    pub header: ChunkHeader,
    pub payload: Bytes,
}

/// Handshake types
#[derive(Debug, Clone, Copy)]
pub enum HandshakeType {
    Client,
    Server,
}

/// Chunk Stream ID constants
pub mod csid {
    pub const PROTOCOL_CONTROL: u32 = 2;
    pub const COMMAND: u32 = 3;
    pub const AUDIO: u32 = 4;
    pub const VIDEO: u32 = 5;
    pub const DATA: u32 = 6;
}

/// Read RTMP handshake (C0/C1/C2 or S0/S1/S2)
pub async fn read_handshake<R: AsyncRead + Unpin>(stream: &mut R) -> Result<HandshakeType> {
    let mut buf = [0u8; 1537];

    // Read C0/S0 (1 byte)
    stream.read_exact(&mut buf[0..1]).await?;
    let version = buf[0];
    if version != 3 {
        return Err(anyhow!("Unsupported RTMP version: {}", version));
    }

    // Read C1/S1 (1536 bytes)
    stream.read_exact(&mut buf[1..]).await?;

    // For simplicity, we'll assume we're reading client handshake
    Ok(HandshakeType::Client)
}

/// Write RTMP handshake response (S0 + S1 only).
///
/// Prefer [`server_handshake`], which completes the full exchange.
pub async fn write_handshake<W: AsyncWrite + Unpin>(stream: &mut W, _type: HandshakeType) -> Result<()> {
    let mut buf = [0u8; 1537];

    // S0/S1 or C0/C1/C2
    buf[0] = 3; // Version

    // Time (4 bytes) - use current time
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u32;
    buf[1..5].copy_from_slice(&now.to_be_bytes());

    // Zero padding (4 bytes)
    buf[5..9].copy_from_slice(&[0, 0, 0, 0]);

    // Random data (1528 bytes)
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut buf[9..]);

    stream.write_all(&buf).await?;
    stream.flush().await?;

    Ok(())
}

/// Perform the complete server side handshake: read C0/C1, send S0/S1,
/// read C2 and echo C1 as S2.
///
/// Supports both "simple handshake" (S1 echoes C1) and "complex handshake"
/// (S1 has fresh random data). Detects which variant the client expects
/// by examining C1: if bytes 5-8 (the second timestamp field) are non-zero,
/// the client is using simple handshake (like ffmpeg).
///
/// For simple handshake, some clients (notably ffmpeg) expect the server to
/// send S0+S1+S2 as a single response (since S1=S2=C1 are both known),
/// then the client responds with C2. We send S0+S1+S2 together for simple
/// handshake, then read C2.
pub async fn server_handshake<S: AsyncRead + AsyncWrite + Unpin>(stream: &mut S) -> Result<()> {
    let mut c0c1 = [0u8; 1537];
    debug!("Reading C0+C1 (1537 bytes)...");
    stream.read_exact(&mut c0c1).await?;
    debug!("Read C0+C1: version={}, C1 first 16 bytes={:02x?}", c0c1[0], &c0c1[1..17]);

    if c0c1[0] != 3 {
        bail!("Unsupported RTMP version: {}", c0c1[0]);
    }

    // Detect handshake type by examining C1 structure:
    // Complex handshake C1: timestamp(4) + zero(4) + random(1528)
    // Simple handshake C1:  timestamp(4) + random(1532) -- no zero field
    // ffmpeg and some other clients send non-zero in bytes 5-8, indicating simple handshake.
    let c1_time = u32::from_be_bytes([c0c1[1], c0c1[2], c0c1[3], c0c1[4]]);
    let c1_time2 = u32::from_be_bytes([c0c1[5], c0c1[6], c0c1[7], c0c1[8]]);
    debug!("C1 time={}, time2={}", c1_time, c1_time2);

    // If second 4 bytes are non-zero, client expects simple handshake
    let use_simple_handshake = c1_time2 != 0;
    debug!("Handshake type: {}", if use_simple_handshake { "simple (S1=C1)" } else { "complex (fresh S1)" });

    if use_simple_handshake {
        // Simple handshake: S0 + S1 + S2 all together (S1=S2=C1)
        debug!("Using simple handshake: sending S0+S1+S2 (S1=S2=C1)");
        let mut s0s1s2 = [0u8; 1537 + 1536];
        s0s1s2[0] = 3; // S0
        s0s1s2[1..1537].copy_from_slice(&c0c1[1..]); // S1 = C1
        s0s1s2[1537..].copy_from_slice(&c0c1[1..]);   // S2 = C1

        debug!("Writing S0+S1+S2 ({} bytes), S0={}, S1 first 16={:02x?}", s0s1s2.len(), s0s1s2[0], &s0s1s2[1..17]);
        stream.write_all(&s0s1s2).await?;
        stream.flush().await?;

        // C2 completes the exchange (should equal S1 = C1)
        let mut c2 = [0u8; 1536];
        debug!("Reading C2 (1536 bytes)...");
        stream.read_exact(&mut c2).await?;
        debug!("Read C2: first 16 bytes={:02x?}", &c2[..16]);
    } else {
        // Complex handshake: standard 3-step
        let mut s0s1 = [0u8; 1537];
        s0s1[0] = 3;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u32;
        s0s1[1..5].copy_from_slice(&now.to_be_bytes());
        s0s1[5..9].copy_from_slice(&[0, 0, 0, 0]);
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut s0s1[9..]);

        debug!("Writing S0+S1 (1537 bytes), S0={}, S1 first 16={:02x?}", s0s1[0], &s0s1[1..17]);
        stream.write_all(&s0s1).await?;
        stream.flush().await?;

        // C2 completes the exchange; should be a copy of S1.
        let mut c2 = [0u8; 1536];
        debug!("Reading C2 (1536 bytes)...");
        stream.read_exact(&mut c2).await?;
        debug!("Read C2: first 16 bytes={:02x?}", &c2[..16]);

        // S2 echoes C1.
        debug!("Writing S2 (echo of C1, 1536 bytes)...");
        stream.write_all(&c0c1[1..]).await?;
        stream.flush().await?;
    }

    Ok(())
}

/// Per-chunk-stream state used for reassembly.
#[derive(Debug, Default)]
struct ChunkStreamState {
    timestamp: u32,
    delta: u32,
    message_length: u32,
    message_type: MessageType,
    message_stream_id: u32,
    extended_timestamp: bool,
    started: bool,
    remaining: u32,
    payload: BytesMut,
}

impl ChunkStreamState {
    fn begin_message(&mut self) {
        self.payload.clear();
        self.remaining = self.message_length;
        self.started = true;
    }
}

/// Stateful RTMP chunk reader.
pub struct ChunkReader {
    chunk_size: u32,
    max_message_size: u32,
    streams: HashMap<ChunkStreamId, ChunkStreamState>,
}

impl Default for ChunkReader {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkReader {
    pub fn new() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            max_message_size: MAX_MESSAGE_SIZE,
            streams: HashMap::new(),
        }
    }

    /// Override the maximum message size accepted from the peer.
    pub fn with_max_message_size(mut self, size: u32) -> Self {
        self.max_message_size = size.max(1);
        self
    }

    pub fn chunk_size(&self) -> u32 {
        self.chunk_size
    }

    /// Read one complete RTMP message, reassembling chunks as needed.
    ///
    /// Protocol control messages are applied to the reader's own state
    /// (chunk size, abort) before they are returned to the caller.
    pub async fn read_message<R: AsyncRead + Unpin>(&mut self, stream: &mut R) -> Result<RtmpMessage> {
        loop {
            let (fmt, csid) = read_basic_header(stream).await?;
            let chunk_size = self.chunk_size;
            let max_message_size = self.max_message_size;

            let completed = {
                let state = self.streams.entry(csid).or_default();
                let continuing = state.started && state.remaining > 0;

                match fmt {
                    // Type 0: full header, 11 bytes.
                    0 => {
                        let timestamp = read_u24_async(stream).await?;
                        let message_length = read_u24_async(stream).await?;
                        let mut type_buf = [0u8; 1];
                        stream.read_exact(&mut type_buf).await?;
                        let mut sid_buf = [0u8; 4];
                        stream.read_exact(&mut sid_buf).await?;

                        if message_length > max_message_size {
                            bail!(
                                "RTMP message too large: {} bytes (limit {})",
                                message_length,
                                max_message_size
                            );
                        }

                        state.message_length = message_length;
                        state.message_type = MessageType::from(type_buf[0]);
                        state.message_stream_id = u32::from_le_bytes(sid_buf);
                        state.extended_timestamp = timestamp == 0xFFFFFF;
                        let ext = if state.extended_timestamp {
                            Some(read_u32_async(stream).await?)
                        } else {
                            None
                        };
                        state.delta = 0;
                        state.timestamp = ext.unwrap_or(timestamp);
                        state.begin_message();
                    }
                    // Type 1: timestamp delta + length + type, 7 bytes.
                    1 => {
                        let timestamp_delta = read_u24_async(stream).await?;
                        let message_length = read_u24_async(stream).await?;
                        let mut type_buf = [0u8; 1];
                        stream.read_exact(&mut type_buf).await?;

                        if message_length > max_message_size {
                            bail!(
                                "RTMP message too large: {} bytes (limit {})",
                                message_length,
                                max_message_size
                            );
                        }

                        state.message_length = message_length;
                        state.message_type = MessageType::from(type_buf[0]);
                        state.extended_timestamp = timestamp_delta == 0xFFFFFF;
                        let ext = if state.extended_timestamp {
                            Some(read_u32_async(stream).await?)
                        } else {
                            None
                        };
                        let delta = ext.unwrap_or(timestamp_delta);
                        state.delta = delta;
                        state.timestamp = state.timestamp.wrapping_add(delta);
                        state.begin_message();
                    }
                    // Type 2: timestamp delta only, 3 bytes.
                    2 => {
                        let timestamp_delta = read_u24_async(stream).await?;
                        state.extended_timestamp = timestamp_delta == 0xFFFFFF;
                        let ext = if state.extended_timestamp {
                            Some(read_u32_async(stream).await?)
                        } else {
                            None
                        };
                        let delta = ext.unwrap_or(timestamp_delta);
                        state.delta = delta;
                        state.timestamp = state.timestamp.wrapping_add(delta);
                        state.begin_message();
                    }
                    // Type 3: no message header.
                    3 => {
                        let ext = if state.extended_timestamp {
                            Some(read_u32_async(stream).await?)
                        } else {
                            None
                        };
                        if !continuing {
                            // First chunk of a new message: the previous delta repeats.
                            let delta = ext.unwrap_or(state.delta);
                            if state.extended_timestamp {
                                state.delta = delta;
                            }
                            state.timestamp = state.timestamp.wrapping_add(delta);
                            state.begin_message();
                        }
                    }
                    _ => unreachable!("fmt is two bits"),
                }

                let to_read = chunk_size.min(state.remaining) as usize;
                if to_read > 0 {
                    let start = state.payload.len();
                    state.payload.resize(start + to_read, 0);
                    stream.read_exact(&mut state.payload[start..]).await?;
                    state.remaining -= to_read as u32;
                }

                if state.remaining == 0 {
                    Some(RtmpMessage {
                        header: ChunkHeader {
                            chunk_stream_id: csid,
                            timestamp: state.timestamp,
                            message_length: state.message_length,
                            message_type: state.message_type,
                            message_stream_id: state.message_stream_id,
                        },
                        payload: std::mem::take(&mut state.payload).freeze(),
                    })
                } else {
                    None
                }
            };

            if let Some(message) = completed {
                match message.header.message_type {
                    MessageType::SetChunkSize => {
                        if message.payload.len() >= 4 {
                            let size = u32::from_be_bytes([
                                message.payload[0],
                                message.payload[1],
                                message.payload[2],
                                message.payload[3],
                            ]);
                            if size == 0 || size > MAX_CHUNK_SIZE {
                                bail!("Peer announced invalid chunk size: {}", size);
                            }
                            self.chunk_size = size;
                        }
                    }
                    MessageType::Abort => {
                        if message.payload.len() >= 4 {
                            let aborted = u32::from_be_bytes([
                                message.payload[0],
                                message.payload[1],
                                message.payload[2],
                                message.payload[3],
                            ]);
                            if let Some(state) = self.streams.get_mut(&aborted) {
                                state.payload.clear();
                                state.remaining = 0;
                            }
                        }
                    }
                    _ => {}
                }

                return Ok(message);
            }
        }
    }
}

/// Write a chunk to stream (single message, default chunk size).
///
/// Prefer a reusable [`ChunkWriter`] if a non-default chunk size is used.
pub async fn write_chunk<W: AsyncWrite + Unpin>(stream: &mut W, message: &RtmpMessage) -> Result<()> {
    ChunkWriter::new().write_message(stream, message).await
}

/// Read a chunk from stream.
///
/// This creates a fresh reader for every call, so it cannot decode compressed
/// headers (fmt 1-3) that refer to an earlier message. Use [`ChunkReader`] for
/// real connections; this helper exists for tests and one-shot messages.
pub async fn read_chunk<R: AsyncRead + Unpin>(stream: &mut R) -> Result<RtmpMessage> {
    ChunkReader::new().read_message(stream).await
}

/// Stateful RTMP chunk writer.
pub struct ChunkWriter {
    chunk_size: u32,
}

impl Default for ChunkWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkWriter {
    pub fn new() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }

    pub fn with_chunk_size(chunk_size: u32) -> Self {
        Self {
            chunk_size: chunk_size.max(1),
        }
    }

    pub fn set_chunk_size(&mut self, chunk_size: u32) {
        self.chunk_size = chunk_size.max(1);
    }

    pub fn chunk_size(&self) -> u32 {
        self.chunk_size
    }

    /// Write a complete message, splitting it into chunks as required.
    pub async fn write_message<W: AsyncWrite + Unpin>(
        &self,
        stream: &mut W,
        message: &RtmpMessage,
    ) -> Result<()> {
        let payload = &message.payload;
        let total_len = payload.len();
        let timestamp = message.header.timestamp;
        let use_extended = timestamp >= 0xFFFFFF;
        let mut sent = 0usize;
        let mut first = true;

        loop {
            let remaining = total_len - sent;
            let chunk_data_len = std::cmp::min(self.chunk_size as usize, remaining);

            // Basic header
            let fmt: u8 = if first { 0 } else { 3 };
            write_basic_header(stream, fmt, message.header.chunk_stream_id).await?;

            if first {
                // Message header (fmt 0): timestamp, length, type, stream id.
                let wire_ts = if use_extended { 0xFFFFFF } else { timestamp };
                write_u24_async(stream, wire_ts).await?;
                write_u24_async(stream, total_len as u32).await?;
                stream.write_all(&[message.header.message_type as u8]).await?;
                stream
                    .write_all(&message.header.message_stream_id.to_le_bytes())
                    .await?;
                if use_extended {
                    stream.write_all(&timestamp.to_be_bytes()).await?;
                }
                first = false;
            } else if use_extended {
                // Extended timestamp is repeated on every chunk of the message.
                stream.write_all(&timestamp.to_be_bytes()).await?;
            }

            stream.write_all(&payload[sent..sent + chunk_data_len]).await?;
            sent += chunk_data_len;

            if sent >= total_len {
                break;
            }
        }

        stream.flush().await?;
        Ok(())
    }
}

async fn read_basic_header<R: AsyncRead + Unpin>(stream: &mut R) -> Result<(u8, ChunkStreamId)> {
    let mut byte = [0u8; 1];
    stream.read_exact(&mut byte).await?;
    let fmt = (byte[0] >> 6) & 0x03;
    let csid = match byte[0] & 0x3F {
        0 => {
            let mut b = [0u8; 1];
            stream.read_exact(&mut b).await?;
            64 + b[0] as u32
        }
        1 => {
            let mut b = [0u8; 2];
            stream.read_exact(&mut b).await?;
            64 + b[0] as u32 + ((b[1] as u32) << 8)
        }
        n => n as u32,
    };
    Ok((fmt, csid))
}

async fn write_basic_header<W: AsyncWrite + Unpin>(
    stream: &mut W,
    fmt: u8,
    csid: ChunkStreamId,
) -> Result<()> {
    let fmt_bits = (fmt & 0x03) << 6;
    if csid < 64 {
        stream.write_all(&[fmt_bits | csid as u8]).await?;
    } else if csid < 320 {
        stream
            .write_all(&[fmt_bits, (csid - 64) as u8])
            .await?;
    } else {
        let value = csid - 64;
        stream
            .write_all(&[fmt_bits | 0x01, (value >> 8) as u8, value as u8])
            .await?;
    }
    Ok(())
}

async fn read_u24_async<R: AsyncRead + Unpin>(stream: &mut R) -> Result<u32> {
    let mut b = [0u8; 3];
    stream.read_exact(&mut b).await?;
    Ok(((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32)
}

async fn read_u32_async<R: AsyncRead + Unpin>(stream: &mut R) -> Result<u32> {
    let mut b = [0u8; 4];
    stream.read_exact(&mut b).await?;
    Ok(u32::from_be_bytes(b))
}

async fn write_u24_async<W: AsyncWrite + Unpin>(stream: &mut W, value: u32) -> Result<()> {
    stream
        .write_all(&[(value >> 16) as u8, (value >> 8) as u8, value as u8])
        .await?;
    Ok(())
}

/// Read U8
pub fn read_u8(buf: &mut Cursor<&[u8]>) -> Result<u8> {
    let mut b = [0u8; 1];
    Read::read_exact(buf, &mut b)?;
    Ok(b[0])
}

/// Read U16 BE
pub fn read_u16_be(buf: &mut Cursor<&[u8]>) -> Result<u16> {
    let mut b = [0u8; 2];
    Read::read_exact(buf, &mut b)?;
    Ok(u16::from_be_bytes(b))
}

/// Read U24 BE
pub fn read_u24_be(buf: &mut Cursor<&[u8]>) -> Result<u32> {
    let mut b = [0u8; 3];
    Read::read_exact(buf, &mut b)?;
    Ok((b[0] as u32) << 16 | (b[1] as u32) << 8 | (b[2] as u32))
}

/// Read U32 BE
pub fn read_u32_be(buf: &mut Cursor<&[u8]>) -> Result<u32> {
    let mut b = [0u8; 4];
    Read::read_exact(buf, &mut b)?;
    Ok(u32::from_be_bytes(b))
}

/// Write U24 BE
pub fn write_u24_be(buf: &mut BytesMut, val: u32) {
    buf.put_u8((val >> 16) as u8);
    buf.put_u8((val >> 8) as u8);
    buf.put_u8(val as u8);
}

/// Write U32 BE
pub fn write_u32_be(buf: &mut BytesMut, val: u32) {
    buf.put_u32(val);
}

/// AMF0 encoding helpers
pub mod amf0 {
    use bytes::{Bytes, BytesMut, BufMut};
    use anyhow::Result;

    pub const NUMBER: u8 = 0x00;
    pub const BOOLEAN: u8 = 0x01;
    pub const STRING: u8 = 0x02;
    pub const OBJECT: u8 = 0x03;
    pub const NULL: u8 = 0x05;
    pub const UNDEFINED: u8 = 0x06;
    pub const ECMA_ARRAY: u8 = 0x08;
    pub const OBJECT_END: u8 = 0x09;
    pub const STRICT_ARRAY: u8 = 0x0A;
    pub const DATE: u8 = 0x0B;
    pub const LONG_STRING: u8 = 0x0C;

    pub fn write_string(buf: &mut BytesMut, s: &str) {
        buf.put_u8(STRING);
        let bytes = s.as_bytes();
        buf.put_u16(bytes.len() as u16);
        buf.put_slice(bytes);
    }

    /// Object keys are a bare u16 length followed by the bytes (no type marker).
    pub fn write_object_key(buf: &mut BytesMut, s: &str) {
        let bytes = s.as_bytes();
        buf.put_u16(bytes.len() as u16);
        buf.put_slice(bytes);
    }

    pub fn write_long_string(buf: &mut BytesMut, s: &str) {
        buf.put_u8(LONG_STRING);
        let bytes = s.as_bytes();
        buf.put_u32(bytes.len() as u32);
        buf.put_slice(bytes);
    }

    pub fn write_number(buf: &mut BytesMut, n: f64) {
        buf.put_u8(NUMBER);
        buf.put_f64(n);
    }

    pub fn write_boolean(buf: &mut BytesMut, b: bool) {
        buf.put_u8(BOOLEAN);
        buf.put_u8(if b { 1 } else { 0 });
    }

    pub fn write_object_start(buf: &mut BytesMut) {
        buf.put_u8(OBJECT);
    }

    pub fn write_object_end(buf: &mut BytesMut) {
        buf.put_u8(0);
        buf.put_u8(0);
        buf.put_u8(OBJECT_END);
    }

    pub fn write_null(buf: &mut BytesMut) {
        buf.put_u8(NULL);
    }

    pub fn write_undefined(buf: &mut BytesMut) {
        buf.put_u8(UNDEFINED);
    }

    /// Write a command message (connect, createStream, publish, etc.)
    /// Format: command_name (String), transaction_id (Number), command_object (Object), [optional_args...]
    pub fn write_command(buf: &mut BytesMut, command: &str, transaction_id: f64, command_object: Option<&dyn Fn(&mut BytesMut)>) {
        write_string(buf, command);
        write_number(buf, transaction_id);
        if let Some(obj_fn) = command_object {
            write_object_start(buf);
            obj_fn(buf);
            write_object_end(buf);
        } else {
            write_null(buf);
        }
    }

    /// Write connect command with standard parameters
    pub fn write_connect_command(buf: &mut BytesMut, app: &str, tc_url: &str) {
        write_command(buf, "connect", 1.0, Some(&|buf| {
            // Command object: AMF0 object property names are length-prefixed
            // without a type marker, unlike string values.
            write_object_key(buf, "app");
            write_string(buf, app);

            write_object_key(buf, "tcUrl");
            write_string(buf, tc_url);

            write_object_key(buf, "type");
            write_string(buf, "nonprivate");

            write_object_key(buf, "flashVer");
            write_string(buf, "FMLE/3.0 (compatible; InstantDelay)");

            write_object_key(buf, "audioCodecs");
            write_number(buf, 250.0);
            write_object_key(buf, "videoCodecs");
            write_number(buf, 252.0);
            write_object_key(buf, "videoFunction");
            write_number(buf, 3.0);
            write_object_key(buf, "capabilities");
            write_number(buf, 3575.0);

            write_object_key(buf, "objectEncoding");
            write_number(buf, 0.0);
        }));
    }

    /// Write createStream command
    pub fn write_create_stream_command(buf: &mut BytesMut) {
        write_command(buf, "createStream", 2.0, None);
    }

    /// Write publish command. The stream name and publish type are positional
    /// arguments, not properties of a command object.
    pub fn write_publish_command(buf: &mut BytesMut, stream_name: &str, publish_type: &str) {
        write_command(buf, "publish", 0.0, None);
        write_string(buf, stream_name);
        write_string(buf, publish_type); // "live", "record", "append"
    }

    /// Write setChunkSize command
    pub fn write_set_chunk_size(buf: &mut BytesMut, size: u32) {
        // This is a protocol control message, not a command
        // Handled separately
    }

    /// Read an AMF0 string (type marker + u16 length + bytes).
    pub fn read_string(payload: &[u8], pos: &mut usize) -> Option<String> {
        if *pos >= payload.len() || payload[*pos] != STRING {
            return None;
        }
        *pos += 1;
        if *pos + 2 > payload.len() {
            return None;
        }
        let len = u16::from_be_bytes([payload[*pos], payload[*pos + 1]]) as usize;
        *pos += 2;
        if *pos + len > payload.len() {
            return None;
        }
        let s = String::from_utf8_lossy(&payload[*pos..*pos + len]).into_owned();
        *pos += len;
        Some(s)
    }

    /// Read an AMF0 number (type marker + f64).
    pub fn read_number(payload: &[u8], pos: &mut usize) -> Option<f64> {
        if *pos >= payload.len() || payload[*pos] != NUMBER {
            return None;
        }
        *pos += 1;
        if *pos + 8 > payload.len() {
            return None;
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&payload[*pos..*pos + 8]);
        *pos += 8;
        Some(f64::from_be_bytes(bytes))
    }

    /// Parse the command name and transaction id at the start of a command.
    pub fn read_command(payload: &[u8]) -> Option<(String, f64)> {
        let mut pos = 0usize;
        let name = read_string(payload, &mut pos)?;
        let transaction_id = read_number(payload, &mut pos).unwrap_or(0.0);
        Some((name, transaction_id))
    }

    /// Extract a named string property from the first AMF0 object in a payload.
    pub fn find_string_property(payload: &[u8], key: &str) -> Option<String> {
        let mut i = 0usize;
        while i + 2 <= payload.len() {
            // Object keys are length-prefixed; scan for a key match.
            let len = u16::from_be_bytes([payload[i], payload[i + 1]]) as usize;
            if len == key.len() && i + 2 + len <= payload.len()
                && &payload[i + 2..i + 2 + len] == key.as_bytes()
            {
                let mut pos = i + 2 + len;
                if pos < payload.len() {
                    match payload[pos] {
                        STRING => return read_string(payload, &mut pos),
                        _ => return None,
                    }
                }
                return None;
            }
            i += 1;
        }
        None
    }

    /// Build the `_result` payload for `connect`.
    pub fn build_connect_result() -> Bytes {
        let mut buf = BytesMut::new();
        write_string(&mut buf, "_result");
        write_number(&mut buf, 1.0);

        write_object_start(&mut buf);
        write_object_key(&mut buf, "fmsVer");
        write_string(&mut buf, "FMS/3,0,1,123");
        write_object_key(&mut buf, "capabilities");
        write_number(&mut buf, 31.0);
        write_object_end(&mut buf);

        write_object_start(&mut buf);
        write_object_key(&mut buf, "level");
        write_string(&mut buf, "status");
        write_object_key(&mut buf, "code");
        write_string(&mut buf, "NetConnection.Connect.Success");
        write_object_key(&mut buf, "description");
        write_string(&mut buf, "Connection succeeded.");
        write_object_key(&mut buf, "objectEncoding");
        write_number(&mut buf, 0.0);
        write_object_end(&mut buf);

        buf.freeze()
    }

    /// Build the `_result` payload for `createStream`.
    pub fn build_create_stream_result(transaction_id: f64, stream_id: f64) -> Bytes {
        let mut buf = BytesMut::new();
        write_string(&mut buf, "_result");
        write_number(&mut buf, transaction_id);
        write_null(&mut buf);
        write_number(&mut buf, stream_id);
        buf.freeze()
    }

    /// Build an `onStatus` payload announcing a published stream.
    pub fn build_publish_start() -> Bytes {
        let mut buf = BytesMut::new();
        write_string(&mut buf, "onStatus");
        write_number(&mut buf, 0.0);
        write_null(&mut buf);
        write_object_start(&mut buf);
        write_object_key(&mut buf, "level");
        write_string(&mut buf, "status");
        write_object_key(&mut buf, "code");
        write_string(&mut buf, "NetStream.Publish.Start");
        write_object_key(&mut buf, "description");
        write_string(&mut buf, "Start publishing");
        write_object_end(&mut buf);
        buf.freeze()
    }

    /// Build a minimal `_result` with a null command object.
    pub fn build_simple_result(transaction_id: f64) -> Bytes {
        let mut buf = BytesMut::new();
        write_string(&mut buf, "_result");
        write_number(&mut buf, transaction_id);
        write_null(&mut buf);
        write_null(&mut buf);
        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_control_and_media_message_types() {
        assert_eq!(MessageType::from(8), MessageType::Audio);
        assert_eq!(MessageType::from(9), MessageType::Video);
        assert_eq!(MessageType::from(18), MessageType::Metadata);
        assert_eq!(MessageType::from(20), MessageType::CommandAMF0);
        assert_eq!(MessageType::from(254), MessageType::Unknown);
    }

    #[test]
    fn reads_big_endian_integer_fields() {
        let mut cursor = Cursor::new(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10][..]);
        assert_eq!(read_u8(&mut cursor).unwrap(), 1);
        assert_eq!(read_u16_be(&mut cursor).unwrap(), 0x0203);
        assert_eq!(read_u24_be(&mut cursor).unwrap(), 0x040506);
        assert_eq!(read_u32_be(&mut cursor).unwrap(), 0x0708090a);
        assert!(read_u8(&mut cursor).is_err());
    }

    #[tokio::test]
    async fn round_trips_one_small_uncompressed_message() {
        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::VIDEO,
                timestamp: 123,
                message_length: 5,
                message_type: MessageType::Video,
                message_stream_id: 1,
            },
            payload: Bytes::from_static(b"frame"),
        };
        let (mut writer, mut reader) = tokio::io::duplex(256);
        write_chunk(&mut writer, &message).await.unwrap();
        let decoded = read_chunk(&mut reader).await.unwrap();
        assert_eq!(decoded.header, message.header);
        assert_eq!(decoded.payload, message.payload);
    }

    #[tokio::test]
    async fn reassembles_a_message_split_across_chunks() {
        let payload = Bytes::from(vec![0xAB; 300]);
        let message = RtmpMessage {
            header: ChunkHeader {
                chunk_stream_id: csid::VIDEO,
                timestamp: 90_000,
                message_length: payload.len() as u32,
                message_type: MessageType::Video,
                message_stream_id: 1,
            },
            payload,
        };
        let expected_header = message.header;
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        let write_task = tokio::spawn(async move {
            ChunkWriter::new().write_message(&mut writer, &message).await.unwrap();
        });

        let mut chunk_reader = ChunkReader::new();
        let decoded = chunk_reader.read_message(&mut reader).await.unwrap();
        write_task.await.unwrap();
        assert_eq!(decoded.header, expected_header);
        assert_eq!(decoded.payload.len(), 300);
    }

    #[tokio::test]
    async fn applies_peer_chunk_size_and_decodes_compressed_headers() {
        let (mut writer, mut reader) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let chunk_size_msg = RtmpMessage {
                header: ChunkHeader {
                    chunk_stream_id: csid::PROTOCOL_CONTROL,
                    timestamp: 0,
                    message_length: 4,
                    message_type: MessageType::SetChunkSize,
                    message_stream_id: 0,
                },
                payload: Bytes::from(4096u32.to_be_bytes().to_vec()),
            };
            ChunkWriter::new().write_message(&mut writer, &chunk_size_msg).await.unwrap();

            // After the chunk size changes, a 200-byte message fits in one chunk.
            let message = RtmpMessage {
                header: ChunkHeader {
                    chunk_stream_id: csid::AUDIO,
                    timestamp: 10,
                    message_length: 200,
                    message_type: MessageType::Audio,
                    message_stream_id: 1,
                },
                payload: Bytes::from(vec![7u8; 200]),
            };
            ChunkWriter::with_chunk_size(4096).write_message(&mut writer, &message).await.unwrap();
        });

        let mut chunk_reader = ChunkReader::new();
        let control = chunk_reader.read_message(&mut reader).await.unwrap();
        assert_eq!(control.header.message_type, MessageType::SetChunkSize);
        assert_eq!(chunk_reader.chunk_size(), 4096);

        let audio = chunk_reader.read_message(&mut reader).await.unwrap();
        assert_eq!(audio.header.message_type, MessageType::Audio);
        assert_eq!(audio.payload.len(), 200);
    }
}