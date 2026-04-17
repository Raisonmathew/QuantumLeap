//! Domain layer - Pure business logic
//!
//! This layer contains:
//! - Value Objects: Immutable objects defined by their attributes
//! - Entities: Objects with identity that can change over time
//! - Domain Services: Business logic that doesn't fit in entities
//! - Domain Events: Things that happened in the domain

// Value Objects
pub mod peer_id;
pub mod session_id;
pub mod nat_type;
pub mod ice_candidate;
pub mod peer_capabilities;

// Entities
pub mod peer;
pub mod session;
pub mod connection;

// Re-exports - Value Objects
pub use peer_id::PeerId;
pub use session_id::SessionId;
pub use nat_type::{NatType, NatCompatibility};
pub use ice_candidate::{CandidateType, IceCandidate};
pub use peer_capabilities::{PeerCapabilities, TransportProtocol};

// Re-exports - Entities
pub use peer::{Peer, PeerState};
pub use session::{Session, SessionState, SessionType};
pub use connection::{Connection, ConnectionAttempt, ConnectionMethod, ConnectionState, ConnectionStrategy};

// Made with Bob
