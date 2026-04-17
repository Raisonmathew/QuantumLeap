//! QLTP Network Protocol Messages
//!
//! This module implements the QLTP network protocol messages as specified in NETWORK_PROTOCOL.md

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Protocol magic number: "QLTP" in ASCII
pub const PROTOCOL_MAGIC: u32 = 0x514C5450;

/// Current protocol version
pub const PROTOCOL_VERSION: u8 = 0x01;

/// Maximum message payload size (16 MB)
pub const MAX_PAYLOAD_SIZE: u32 = 16 * 1024 * 1024;

/// Default chunk size (4 KB)
pub const DEFAULT_CHUNK_SIZE: u32 = 4096;

/// Protocol capabilities bitfield
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities(pub u32);

impl Capabilities {
    pub const COMPRESSION_LZ4: u32 = 1 << 0;
    pub const COMPRESSION_ZSTD: u32 = 1 << 1;
    pub const DEDUPLICATION: u32 = 1 << 2;
    pub const RESUME: u32 = 1 << 3;
    pub const TLS: u32 = 1 << 4;
    pub const QUIC: u32 = 1 << 5;
    pub const PARALLEL_STREAMS: u32 = 1 << 6;
    pub const DELTA_ENCODING: u32 = 1 << 7;

    pub fn new() -> Self {
        Self(0)
    }

    pub fn with_compression_lz4(mut self) -> Self {
        self.0 |= Self::COMPRESSION_LZ4;
        self
    }

    pub fn with_compression_zstd(mut self) -> Self {
        self.0 |= Self::COMPRESSION_ZSTD;
        self
    }

    pub fn with_deduplication(mut self) -> Self {
        self.0 |= Self::DEDUPLICATION;
        self
    }

    pub fn with_resume(mut self) -> Self {
        self.0 |= Self::RESUME;
        self
    }

    pub fn with_tls(mut self) -> Self {
        self.0 |= Self::TLS;
        self
    }

    pub fn with_parallel_streams(mut self) -> Self {
        self.0 |= Self::PARALLEL_STREAMS;
        self
    }

    pub fn has(&self, capability: u32) -> bool {
        (self.0 & capability) != 0
    }

    pub fn default_client() -> Self {
        Self::new()
            .with_compression_lz4()
            .with_compression_zstd()
            .with_deduplication()
            .with_resume()
            .with_parallel_streams()
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// Message types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessageType {
    Hello = 0x01,
    Welcome = 0x02,
    TransferStart = 0x03,
    TransferAck = 0x04,
    ChunkData = 0x05,
    ChunkAck = 0x06,
    TransferEnd = 0x07,
    TransferComplete = 0x08,
    Error = 0x09,
    Ping = 0x0A,
    Pong = 0x0B,
    ResumeRequest = 0x0C,
    ResumeAck = 0x0D,
    Metadata = 0x0E,
    Goodbye = 0x0F,
}

impl MessageType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::Hello),
            0x02 => Some(Self::Welcome),
            0x03 => Some(Self::TransferStart),
            0x04 => Some(Self::TransferAck),
            0x05 => Some(Self::ChunkData),
            0x06 => Some(Self::ChunkAck),
            0x07 => Some(Self::TransferEnd),
            0x08 => Some(Self::TransferComplete),
            0x09 => Some(Self::Error),
            0x0A => Some(Self::Ping),
            0x0B => Some(Self::Pong),
            0x0C => Some(Self::ResumeRequest),
            0x0D => Some(Self::ResumeAck),
            0x0E => Some(Self::Metadata),
            0x0F => Some(Self::Goodbye),
            _ => None,
        }
    }
}

/// Error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ErrorCode {
    Success = 0x00,
    ProtocolError = 0x01,
    InvalidMessage = 0x02,
    UnsupportedVersion = 0x03,
    AuthenticationFailed = 0x04,
    InsufficientSpace = 0x05,
    FileNotFound = 0x06,
    PermissionDenied = 0x07,
    ChecksumMismatch = 0x08,
    Timeout = 0x09,
    ConnectionLost = 0x0A,
    TransferAborted = 0x0B,
    ResourceExhausted = 0x0C,
}

/// Compression algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CompressionType {
    None = 0x00,
    Lz4 = 0x01,
    Zstd = 0x02,
}

/// Hash algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum HashAlgorithm {
    Sha256 = 0x01,
}

/// Chunk flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkFlags(pub u8);

impl ChunkFlags {
    pub const COMPRESSED: u8 = 1 << 0;
    pub const DEDUPLICATED: u8 = 1 << 1;
    pub const LAST_CHUNK: u8 = 1 << 2;
    pub const ENCRYPTED: u8 = 1 << 3;

    pub fn new() -> Self {
        Self(0)
    }

    pub fn with_compressed(mut self) -> Self {
        self.0 |= Self::COMPRESSED;
        self
    }

    pub fn with_deduplicated(mut self) -> Self {
        self.0 |= Self::DEDUPLICATED;
        self
    }

    pub fn with_last_chunk(mut self) -> Self {
        self.0 |= Self::LAST_CHUNK;
        self
    }

    pub fn is_compressed(&self) -> bool {
        (self.0 & Self::COMPRESSED) != 0
    }

    pub fn is_deduplicated(&self) -> bool {
        (self.0 & Self::DEDUPLICATED) != 0
    }

    pub fn is_last_chunk(&self) -> bool {
        (self.0 & Self::LAST_CHUNK) != 0
    }
}

impl Default for ChunkFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Base message header (30 bytes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    pub message_type: MessageType,
    pub flags: u8,
    pub sequence_number: u32,
    pub session_id: Uuid,
    pub payload_length: u32,
    pub checksum: u32,
}

impl MessageHeader {
    pub fn new(message_type: MessageType, session_id: Uuid, payload_length: u32) -> Self {
        Self {
            message_type,
            flags: 0,
            sequence_number: 0,
            session_id,
            payload_length,
            checksum: 0,
        }
    }

    pub fn size() -> usize {
        30
    }
}

/// HELLO message (40 bytes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloMessage {
    pub magic: u32,
    pub version: u8,
    pub flags: u8,
    pub client_id: Uuid,
    pub capabilities: u32,
    pub timestamp: u64,
}

impl HelloMessage {
    pub fn new(client_id: Uuid, capabilities: Capabilities) -> Self {
        Self {
            magic: PROTOCOL_MAGIC,
            version: PROTOCOL_VERSION,
            flags: 0,
            client_id,
            capabilities: capabilities.0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// WELCOME message (60 bytes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WelcomeMessage {
    pub magic: u32,
    pub version: u8,
    pub status: ErrorCode,
    pub server_id: Uuid,
    pub session_id: Uuid,
    pub max_chunk_size: u32,
    pub max_parallel_streams: u16,
    pub timestamp: u64,
}

impl WelcomeMessage {
    pub fn new(server_id: Uuid, session_id: Uuid) -> Self {
        Self {
            magic: PROTOCOL_MAGIC,
            version: PROTOCOL_VERSION,
            status: ErrorCode::Success,
            server_id,
            session_id,
            max_chunk_size: DEFAULT_CHUNK_SIZE,
            max_parallel_streams: 4,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// TRANSFER_START message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferStartMessage {
    pub transfer_id: Uuid,
    pub file_name: String,
    pub file_size: u64,
    pub total_chunks: u32,
    pub chunk_size: u32,
    pub compression: CompressionType,
    pub hash_algorithm: HashAlgorithm,
    pub file_hash: [u8; 32],
    pub metadata: Option<String>,
}

impl TransferStartMessage {
    pub fn new(
        file_name: String,
        file_size: u64,
        total_chunks: u32,
        chunk_size: u32,
        file_hash: [u8; 32],
    ) -> Self {
        Self {
            transfer_id: Uuid::new_v4(),
            file_name,
            file_size,
            total_chunks,
            chunk_size,
            compression: CompressionType::Lz4,
            hash_algorithm: HashAlgorithm::Sha256,
            file_hash,
            metadata: None,
        }
    }
}

/// TRANSFER_ACK message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferAckMessage {
    pub transfer_id: Uuid,
    pub status: ErrorCode,
    pub resume_offset: u64,
    pub available_space: u64,
    pub preferred_chunk_size: u32,
}

/// CHUNK_DATA message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkDataMessage {
    pub transfer_id: Uuid,
    pub chunk_index: u32,
    pub chunk_offset: u64,
    pub original_size: u32,
    pub compressed_size: u32,
    pub chunk_hash: [u8; 32],
    pub compression_type: CompressionType,
    pub flags: u8,
    pub data: Vec<u8>,
}

impl ChunkDataMessage {
    pub fn new(
        transfer_id: Uuid,
        chunk_index: u32,
        chunk_offset: u64,
        data: Vec<u8>,
        chunk_hash: [u8; 32],
    ) -> Self {
        let original_size = data.len() as u32;
        Self {
            transfer_id,
            chunk_index,
            chunk_offset,
            original_size,
            compressed_size: original_size,
            chunk_hash,
            compression_type: CompressionType::None,
            flags: 0,
            data,
        }
    }
}

/// CHUNK_ACK message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkAckMessage {
    pub transfer_id: Uuid,
    pub chunk_index: u32,
    pub status: ErrorCode,
    pub received_size: u32,
    pub timestamp: u64,
}

impl ChunkAckMessage {
    pub fn new(transfer_id: Uuid, chunk_index: u32, received_size: u32) -> Self {
        Self {
            transfer_id,
            chunk_index,
            status: ErrorCode::Success,
            received_size,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// TRANSFER_END message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferEndMessage {
    pub transfer_id: Uuid,
    pub total_bytes_sent: u64,
    pub total_chunks: u32,
    pub compression_ratio: f32,
    pub transfer_duration_ms: u64,
    pub file_hash: [u8; 32],
}

/// TRANSFER_COMPLETE message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferCompleteMessage {
    pub transfer_id: Uuid,
    pub status: ErrorCode,
    pub total_bytes_received: u64,
    pub verified_hash: [u8; 32],
    pub storage_path: String,
}

/// ERROR message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub error_code: ErrorCode,
    pub message: String,
    pub context: Option<String>,
}

impl ErrorMessage {
    pub fn new(error_code: ErrorCode, message: String) -> Self {
        Self {
            error_code,
            message,
            context: None,
        }
    }
}

/// RESUME_REQUEST message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeRequestMessage {
    pub transfer_id: Uuid,
    pub last_chunk_index: u32,
    pub last_chunk_offset: u64,
    pub partial_file_hash: [u8; 32],
}

/// RESUME_ACK message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeAckMessage {
    pub transfer_id: Uuid,
    pub resume_from_chunk: u32,
    pub resume_from_offset: u64,
    pub status: ErrorCode,
}

/// Complete protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Hello(HelloMessage),
    Welcome(WelcomeMessage),
    TransferStart(TransferStartMessage),
    TransferAck(TransferAckMessage),
    ChunkData(ChunkDataMessage),
    ChunkAck(ChunkAckMessage),
    TransferEnd(TransferEndMessage),
    TransferComplete(TransferCompleteMessage),
    Error(ErrorMessage),
    Ping,
    Pong,
    ResumeRequest(ResumeRequestMessage),
    ResumeAck(ResumeAckMessage),
    Goodbye,
}

impl Message {
    pub fn message_type(&self) -> MessageType {
        match self {
            Message::Hello(_) => MessageType::Hello,
            Message::Welcome(_) => MessageType::Welcome,
            Message::TransferStart(_) => MessageType::TransferStart,
            Message::TransferAck(_) => MessageType::TransferStart,
            Message::ChunkData(_) => MessageType::ChunkData,
            Message::ChunkAck(_) => MessageType::ChunkAck,
            Message::TransferEnd(_) => MessageType::TransferEnd,
            Message::TransferComplete(_) => MessageType::TransferEnd,
            Message::Error(_) => MessageType::Error,
            Message::Ping => MessageType::Ping,
            Message::Pong => MessageType::Pong,
            Message::ResumeRequest(_) => MessageType::ResumeRequest,
            Message::ResumeAck(_) => MessageType::ResumeAck,
            Message::Goodbye => MessageType::Goodbye,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities() {
        let caps = Capabilities::default_client();
        assert!(caps.has(Capabilities::COMPRESSION_LZ4));
        assert!(caps.has(Capabilities::COMPRESSION_ZSTD));
        assert!(caps.has(Capabilities::DEDUPLICATION));
        assert!(caps.has(Capabilities::RESUME));
        assert!(!caps.has(Capabilities::TLS));
    }

    #[test]
    fn test_chunk_flags() {
        let flags = ChunkFlags::new()
            .with_compressed()
            .with_last_chunk();
        assert!(flags.is_compressed());
        assert!(flags.is_last_chunk());
        assert!(!flags.is_deduplicated());
    }

    #[test]
    fn test_hello_message() {
        let client_id = Uuid::new_v4();
        let caps = Capabilities::default_client();
        let hello = HelloMessage::new(client_id, caps);
        
        assert_eq!(hello.magic, PROTOCOL_MAGIC);
        assert_eq!(hello.version, PROTOCOL_VERSION);
        assert_eq!(hello.client_id, client_id);
    }

    #[test]
    fn test_message_type_conversion() {
        assert_eq!(MessageType::from_u8(0x01), Some(MessageType::Hello));
        assert_eq!(MessageType::from_u8(0x04), Some(MessageType::TransferAck));
        assert_eq!(MessageType::from_u8(0x05), Some(MessageType::ChunkData));
        assert_eq!(MessageType::from_u8(0xFF), None);
    }
}

// Made with Bob