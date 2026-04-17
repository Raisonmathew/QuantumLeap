//! STUN (Session Traversal Utilities for NAT) Implementation
//!
//! RFC 5389 compliant STUN server for NAT traversal and public address discovery.
//!
//! ## Features
//! - Binding Request/Response handling
//! - XOR-MAPPED-ADDRESS attribute
//! - MESSAGE-INTEGRITY and FINGERPRINT
//! - NAT type detection
//! - UDP and TCP transport

pub mod message;
pub mod attributes;
pub mod server;
pub mod codec;

pub use message::{StunMessage, StunMessageType, StunClass, StunMethod};
pub use attributes::{StunAttribute, MappedAddress};
pub use server::{StunServer, StunServerConfig};
pub use codec::StunCodec;

/// STUN magic cookie (0x2112A442)
pub const MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN message header size (20 bytes)
pub const HEADER_SIZE: usize = 20;

/// Transaction ID size (12 bytes)
pub const TRANSACTION_ID_SIZE: usize = 12;

// Made with Bob
