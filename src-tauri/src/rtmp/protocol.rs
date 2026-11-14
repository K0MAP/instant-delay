//! RTMP Protocol implementation

use std::io::{Cursor, Read};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bytes::{Bytes, BytesMut, BufMut};

pub type ChunkStreamId = u32;
use tracing::{trace, debug};
use anyhow::{Result, anyhow};

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
    CommandAMF0 = 20,
    SharedObjectAMF3 = 16,
    SharedObjectAMF0 = 19,
    Aggregate = 22,
    Unknown = 255,
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
pub async fn read_handshake<R: tokio::io::AsyncRead + Unpin>(stream: &mut R) -> Result<HandshakeType> {
    let mut buf = [0u8; 1537];

    // Read C0/S0 (1 byte)
    stream.read_exact(&mut buf[0..1]).await?;
    let version = buf[0];
    if version != 3 {
        return Err(anyhow!("Unsupported RTMP version: {}", version));
    }

    // Read C1/S1 (1536 bytes)
    stream.read_exact(&mut buf[1..]).await?;

    // Determine if client or server based on time field
    // Client sends timestamp in first 4 bytes, server echoes
    let time = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);

    // For simplicity, we'll assume we're reading client handshake
    Ok(HandshakeType::Client)
}

/// Write RTMP handshake response
pub async fn write_handshake<W: tokio::io::AsyncWrite + Unpin>(stream: &mut W, _type: HandshakeType) -> Result<()> {
    let mut buf = [0u8; 1537];

    // S0/S1/S2 or C0/C1/C2
    buf[0] = 3; // Version

    // Time (4 bytes) - use current time
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u32;
    buf[1..5].copy_from_slice(&now.to_be_bytes());

    // Zero padding (4 bytes)
    buf[5..9].copy_from_slice(&[0, 0, 0, 0]);

    // Random data (1528 bytes) - simplified to zeros
    // In production, use proper random data

    stream.write_all(&buf).await?;
    stream.flush().await?;

    Ok(())
}

/// Read a chunk from stream
pub async fn read_chunk<R: tokio::io::AsyncRead + Unpin>(stream: &mut R) -> Result<RtmpMessage> {
    // Read basic header (1-3 bytes)
    let mut basic_header = [0u8; 3];
    stream.read_exact(&mut basic_header[0..1]).await?;

    let fmt = (basic_header[0] >> 6) & 0x03;
    let mut csid = (basic_header[0] & 0x3F) as u32;

    if csid == 0 {
        // 2-byte CSID
        stream.read_exact(&mut basic_header[1..2]).await?;
        csid = 64 + basic_header[1] as u32;
    } else if csid == 1 {
        // 3-byte CSID
        stream.read_exact(&mut basic_header[1..3]).await?;
        csid = 64 + (basic_header[2] as u32) + ((basic_header[1] as u32) << 8);
    }

    // Read message header based on fmt
    let (timestamp, msg_length, msg_type, msg_stream_id) = match fmt {
        0 => {
            // 11 bytes: timestamp(3) + length(3) + type(1) + stream_id(4)
            let mut hdr = [0u8; 11];
            stream.read_exact(&mut hdr).await?;
            let timestamp = u32::from_be_bytes([0, hdr[0], hdr[1], hdr[2]]);
            let msg_length = u32::from_be_bytes([0, hdr[3], hdr[4], hdr[5]]);
            let msg_type = MessageType::from(hdr[6]);
            let msg_stream_id = u32::from_le_bytes([hdr[7], hdr[8], hdr[9], hdr[10]]);
            (timestamp, msg_length, msg_type, msg_stream_id)
        }
        1 => {
            // 7 bytes: timestamp_delta(3) + length(3) + type(1)
            let mut hdr = [0u8; 7];
            stream.read_exact(&mut hdr).await?;
            let timestamp_delta = u32::from_be_bytes([0, hdr[0], hdr[1], hdr[2]]);
            let msg_length = u32::from_be_bytes([0, hdr[3], hdr[4], hdr[5]]);
            let msg_type = MessageType::from(hdr[6]);
            (timestamp_delta, msg_length, msg_type, 0) // stream_id same as previous
        }
        2 => {
            // 3 bytes: timestamp_delta(3)
            let mut hdr = [0u8; 3];
            stream.read_exact(&mut hdr).await?;
            let timestamp_delta = u32::from_be_bytes([0, hdr[0], hdr[1], hdr[2]]);
            (timestamp_delta, 0, MessageType::Unknown, 0) // length and type same as previous
        }
        3 => {
            // 0 bytes: all same as previous
            (0, 0, MessageType::Unknown, 0)
        }
        _ => return Err(anyhow!("Invalid fmt: {}", fmt)),
    };

    // Read extended timestamp if needed
    let final_timestamp = if timestamp >= 0xFFFFFF {
        let mut ext = [0u8; 4];
        stream.read_exact(&mut ext).await?;
        u32::from_be_bytes(ext)
    } else {
        timestamp
    };

    // Read payload
    let mut payload = BytesMut::with_capacity(msg_length as usize);
    payload.resize(msg_length as usize, 0);
    stream.read_exact(&mut payload).await?;

    Ok(RtmpMessage {
        header: ChunkHeader {
            chunk_stream_id: csid,
            timestamp: final_timestamp,
            message_length: msg_length,
            message_type: msg_type,
            message_stream_id: msg_stream_id,
        },
        payload: payload.freeze(),
    })
}

/// Write a chunk to stream
pub async fn write_chunk<W: tokio::io::AsyncWrite + Unpin>(stream: &mut W, message: &RtmpMessage) -> Result<()> {
    let chunk_size = 128; // Default chunk size
    let payload = &message.payload;
    let total_len = payload.len();
    let mut sent = 0;
    let mut first = true;

    while sent < total_len {
        let remaining = total_len - sent;
        let chunk_data_len = std::cmp::min(chunk_size, remaining);

        // Write basic header
        let csid = message.header.chunk_stream_id;
        if csid <= 63 {
            stream.write_all(&[(0 << 6) | (csid as u8)]).await?;
        } else if csid <= 319 {
            stream.write_all(&[0x00, (csid - 64) as u8]).await?;
        } else {
            stream.write_all(&[0x01, ((csid - 64) >> 8) as u8, (csid - 64) as u8]).await?;
        }

        // Write message header (fmt 0 for first chunk, fmt 3 for continuation)
        if first {
            let timestamp = message.header.timestamp.min(0xFFFFFF);
            let mut hdr = [0u8; 11];
            hdr[0..3].copy_from_slice(&(timestamp as u32).to_be_bytes()[1..]);
            hdr[3..6].copy_from_slice(&(message.header.message_length as u32).to_be_bytes()[1..]);
            hdr[6] = message.header.message_type as u8;
            hdr[7..11].copy_from_slice(&message.header.message_stream_id.to_le_bytes());
            stream.write_all(&hdr).await?;

            if timestamp >= 0xFFFFFF {
                stream.write_all(&timestamp.to_be_bytes()).await?;
            }

            first = false;
        } else {
            // fmt 3 - no header
            stream.write_all(&[(3 << 6) | (csid as u8 & 0x3F)]).await?;
        }

        // Write chunk data
        stream.write_all(&payload[sent..sent + chunk_data_len]).await?;
        sent += chunk_data_len;
    }

    stream.flush().await?;
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
    use bytes::{BytesMut, BufMut};
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
            // Command object with connection parameters
            write_string(buf, "app");
            write_string(buf, app);

            write_string(buf, "tcUrl");
            write_string(buf, tc_url);

            write_string(buf, "type");
            write_string(buf, "nonprivate");

            write_string(buf, "flashVer");
            write_string(buf, "FMLE/3.0 (compatible; InstantDelay)");

            write_number(buf, 250.0); // audioCodecs
            write_number(buf, 252.0); // videoCodecs
            write_number(buf, 3.0);   // videoFunction
            write_number(buf, 3575.0); // capabilities
        }));
    }

    /// Write createStream command
    pub fn write_create_stream_command(buf: &mut BytesMut) {
        write_command(buf, "createStream", 2.0, None);
    }

    /// Write publish command
    pub fn write_publish_command(buf: &mut BytesMut, stream_name: &str, publish_type: &str) {
        write_command(buf, "publish", 3.0, Some(&|buf| {
            write_string(buf, stream_name);
            write_string(buf, publish_type); // "live", "record", "append"
        }));
    }

    /// Write setChunkSize command
    pub fn write_set_chunk_size(buf: &mut BytesMut, size: u32) {
        // This is a protocol control message, not a command
        // Handled separately
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
}