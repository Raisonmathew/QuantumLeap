//! Signaling Protocol Messages
//!
//! Defines the WebSocket message protocol for peer-to-peer signaling.
//! Messages are JSON-encoded for human readability and debugging.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::domain::{NatType, ConnectionMethod};

/// Signaling message from client to server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalingMessage {
    /// Register a peer with the relay server
    Register {
        peer_id: Uuid,
        public_addr: SocketAddr,
        nat_type: NatType,
        capabilities: Vec<String>,
    },

    /// Unregister a peer
    Unregister {
        peer_id: Uuid,
    },

    /// Initiate a session with another peer
    InitiateSession {
        initiator_id: Uuid,
        responder_id: Uuid,
    },

    /// Accept a session invitation
    AcceptSession {
        session_id: Uuid,
        responder_id: Uuid,
    },

    /// Reject a session invitation
    RejectSession {
        session_id: Uuid,
        responder_id: Uuid,
        reason: String,
    },

    /// Initiate connection attempts
    InitiateConnection {
        session_id: Uuid,
        local_peer_id: Uuid,
        remote_peer_id: Uuid,
    },

    /// Update connection status
    UpdateConnection {
        connection_id: Uuid,
        connection_method: ConnectionMethod,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    },

    /// Heartbeat to keep connection alive
    Heartbeat {
        peer_id: Uuid,
    },

    /// Query peer information
    QueryPeer {
        peer_id: Uuid,
    },

    /// Query session information
    QuerySession {
        session_id: Uuid,
    },

    /// Query connection information
    QueryConnection {
        connection_id: Uuid,
    },
}

/// Signaling response from server to client
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalingResponse {
    /// Registration successful
    Registered {
        peer_id: Uuid,
        server_time: i64,
    },

    /// Unregistration successful
    Unregistered {
        peer_id: Uuid,
    },

    /// Session created
    SessionCreated {
        session_id: Uuid,
        initiator_id: Uuid,
        responder_id: Uuid,
    },

    /// Session invitation (sent to responder)
    SessionInvitation {
        session_id: Uuid,
        initiator_id: Uuid,
        initiator_addr: SocketAddr,
        initiator_nat: NatType,
    },

    /// Session accepted
    SessionAccepted {
        session_id: Uuid,
        responder_id: Uuid,
        responder_addr: SocketAddr,
        responder_nat: NatType,
    },

    /// Session rejected
    SessionRejected {
        session_id: Uuid,
        reason: String,
    },

    /// Connection initiated
    ConnectionInitiated {
        connection_id: Uuid,
        session_id: Uuid,
        recommended_methods: Vec<ConnectionMethod>,
    },

    /// Connection updated
    ConnectionUpdated {
        connection_id: Uuid,
        connection_method: ConnectionMethod,
        status: String,
    },

    /// Connection established
    ConnectionEstablished {
        connection_id: Uuid,
        connection_method: ConnectionMethod,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    },

    /// Heartbeat acknowledgment
    HeartbeatAck {
        peer_id: Uuid,
        server_time: i64,
    },

    /// Peer information
    PeerInfo {
        peer_id: Uuid,
        public_addr: SocketAddr,
        nat_type: NatType,
        is_online: bool,
        capabilities: Vec<String>,
    },

    /// Session information
    SessionInfo {
        session_id: Uuid,
        initiator_id: Uuid,
        responder_id: Uuid,
        status: String,
        created_at: i64,
    },

    /// Connection information
    ConnectionInfo {
        connection_id: Uuid,
        session_id: Uuid,
        connection_method: ConnectionMethod,
        status: String,
        local_addr: Option<SocketAddr>,
        remote_addr: Option<SocketAddr>,
    },

    /// Error response
    Error {
        code: String,
        message: String,
        details: Option<String>,
    },
}

impl SignalingMessage {
    /// Get the peer ID associated with this message, if any
    pub fn peer_id(&self) -> Option<Uuid> {
        match self {
            Self::Register { peer_id, .. } => Some(*peer_id),
            Self::Unregister { peer_id } => Some(*peer_id),
            Self::InitiateSession { initiator_id, .. } => Some(*initiator_id),
            Self::AcceptSession { responder_id, .. } => Some(*responder_id),
            Self::RejectSession { responder_id, .. } => Some(*responder_id),
            Self::InitiateConnection { local_peer_id, .. } => Some(*local_peer_id),
            Self::UpdateConnection { .. } => None,
            Self::Heartbeat { peer_id } => Some(*peer_id),
            Self::QueryPeer { peer_id } => Some(*peer_id),
            Self::QuerySession { .. } => None,
            Self::QueryConnection { .. } => None,
        }
    }

    /// Get the session ID associated with this message, if any
    pub fn session_id(&self) -> Option<Uuid> {
        match self {
            Self::AcceptSession { session_id, .. } => Some(*session_id),
            Self::RejectSession { session_id, .. } => Some(*session_id),
            Self::InitiateConnection { session_id, .. } => Some(*session_id),
            Self::QuerySession { session_id } => Some(*session_id),
            _ => None,
        }
    }

    /// Validate message fields
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Register { capabilities, .. } => {
                if capabilities.len() > 100 {
                    return Err("Too many capabilities".to_string());
                }
                Ok(())
            }
            Self::RejectSession { reason, .. } => {
                if reason.len() > 1000 {
                    return Err("Rejection reason too long".to_string());
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl SignalingResponse {
    /// Create an error response
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Create an error response with details
    pub fn error_with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
            details: Some(details.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_message_serialization() {
        let peer_id = Uuid::new_v4();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        
        let msg = SignalingMessage::Register {
            peer_id,
            public_addr: addr,
            nat_type: NatType::FullCone,
            capabilities: vec!["quic".to_string()],
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: SignalingMessage = serde_json::from_str(&json).unwrap();
        
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_response_serialization() {
        let peer_id = Uuid::new_v4();
        
        let resp = SignalingResponse::Registered {
            peer_id,
            server_time: 1234567890,
        };

        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: SignalingResponse = serde_json::from_str(&json).unwrap();
        
        assert_eq!(resp, deserialized);
    }

    #[test]
    fn test_message_peer_id() {
        let peer_id = Uuid::new_v4();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        
        let msg = SignalingMessage::Register {
            peer_id,
            public_addr: addr,
            nat_type: NatType::FullCone,
            capabilities: vec![],
        };

        assert_eq!(msg.peer_id(), Some(peer_id));
    }

    #[test]
    fn test_message_validation() {
        let peer_id = Uuid::new_v4();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        
        // Valid message
        let msg = SignalingMessage::Register {
            peer_id,
            public_addr: addr,
            nat_type: NatType::FullCone,
            capabilities: vec!["quic".to_string()],
        };
        assert!(msg.validate().is_ok());

        // Too many capabilities
        let msg = SignalingMessage::Register {
            peer_id,
            public_addr: addr,
            nat_type: NatType::FullCone,
            capabilities: (0..101).map(|i| format!("cap{}", i)).collect(),
        };
        assert!(msg.validate().is_err());
    }

    #[test]
    fn test_error_response() {
        let err = SignalingResponse::error("NOT_FOUND", "Peer not found");
        
        match err {
            SignalingResponse::Error { code, message, details } => {
                assert_eq!(code, "NOT_FOUND");
                assert_eq!(message, "Peer not found");
                assert_eq!(details, None);
            }
            _ => panic!("Expected error response"),
        }
    }

    #[test]
    fn test_error_response_with_details() {
        let err = SignalingResponse::error_with_details(
            "INVALID_STATE",
            "Session in wrong state",
            "Expected Pending, got Established"
        );
        
        match err {
            SignalingResponse::Error { code, message, details } => {
                assert_eq!(code, "INVALID_STATE");
                assert_eq!(message, "Session in wrong state");
                assert_eq!(details, Some("Expected Pending, got Established".to_string()));
            }
            _ => panic!("Expected error response"),
        }
    }
}

// Made with Bob
