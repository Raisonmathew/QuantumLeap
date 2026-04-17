//! TURN (Traversal Using Relays around NAT) Implementation
//!
//! RFC 5766 compliant TURN server for relaying traffic when direct P2P fails.
//!
//! ## Features
//! - Allocation management (Allocate, Refresh, Release)
//! - Permission management (CreatePermission)
//! - Channel binding (ChannelBind)
//! - Data relay (Send, Data indications)
//! - UDP, TCP, and TLS transport
//! - Long-term credential authentication

pub mod attributes;
pub mod allocation;
pub mod server;

pub use attributes::{TurnAttribute, Lifetime, RequestedTransport};
pub use allocation::{Allocation, AllocationManager, Permission, Channel};
pub use server::{TurnServer, TurnServerConfig};

/// Default allocation lifetime (10 minutes)
pub const DEFAULT_LIFETIME: u32 = 600;

/// Maximum allocation lifetime (1 hour)
pub const MAX_LIFETIME: u32 = 3600;

/// Minimum allocation lifetime (30 seconds)
pub const MIN_LIFETIME: u32 = 30;

/// Channel number range: 0x4000 - 0x7FFF
pub const CHANNEL_MIN: u16 = 0x4000;
pub const CHANNEL_MAX: u16 = 0x7FFF;

// Made with Bob