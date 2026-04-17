//! Adapters Layer - Protocol Handlers and Message Routing
//!
//! This layer adapts external protocols (WebSocket messages) to internal
//! application service calls. It handles:
//! - Message serialization/deserialization
//! - Protocol validation
//! - Routing to appropriate services
//! - Response formatting

pub mod protocol;
pub mod handlers;

pub use protocol::{SignalingMessage, SignalingResponse};
pub use handlers::MessageHandler;

// Made with Bob
