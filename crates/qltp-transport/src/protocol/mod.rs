//! QLTP Protocol Layer
//!
//! This module provides the protocol implementation for QLTP, including:
//! - Message definitions and types
//! - Message codec for serialization/deserialization
//! - Protocol constants and capabilities

pub mod codec;
pub mod messages;
pub mod types;

// Re-export commonly used types
pub use codec::QltpCodec;
pub use messages::{
    Capabilities, ChunkAckMessage, ChunkDataMessage, ChunkFlags, CompressionType, ErrorCode,
    ErrorMessage, HashAlgorithm, HelloMessage, Message, MessageHeader, MessageType,
    ResumeAckMessage, ResumeRequestMessage, TransferAckMessage, TransferCompleteMessage,
    TransferEndMessage, TransferStartMessage, WelcomeMessage, DEFAULT_CHUNK_SIZE,
    MAX_PAYLOAD_SIZE, PROTOCOL_MAGIC, PROTOCOL_VERSION,
};
pub use types::{ProgressCallback, TransferConfig, TransferProgress, TransferStats};

// Made with Bob