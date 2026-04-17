//! Domain layer - Business entities and value objects

mod session;
mod transport_type;
mod session_state;
mod transport_stats;
mod backend_capabilities;
mod connection;

pub use session::{SessionConfig, SessionId, TransportSession};
pub use transport_type::TransportType;
pub use session_state::SessionState;
pub use transport_stats::TransportStats;
pub use backend_capabilities::{BackendCapabilities, Platform};
pub use connection::TransportConnection;

// Made with Bob
