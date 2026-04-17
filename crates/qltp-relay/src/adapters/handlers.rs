//! Message Handlers - Route protocol messages to repository operations
//!
//! This module implements the adapter pattern, translating external
//! protocol messages into domain operations via repositories.

use std::sync::Arc;
use chrono::Utc;

use crate::{
    adapters::protocol::{SignalingMessage, SignalingResponse},
    domain::{Peer, Session, Connection, PeerId, SessionId},
    error::{Error, Result},
    ports::{PeerRepository, SessionRepository, ConnectionRepository},
};

/// Message handler that routes signaling messages to appropriate repositories
pub struct MessageHandler {
    peer_repo: Arc<dyn PeerRepository>,
    session_repo: Arc<dyn SessionRepository>,
    connection_repo: Arc<dyn ConnectionRepository>,
}

impl MessageHandler {
    /// Create a new message handler
    pub fn new(
        peer_repo: Arc<dyn PeerRepository>,
        session_repo: Arc<dyn SessionRepository>,
        connection_repo: Arc<dyn ConnectionRepository>,
    ) -> Self {
        Self {
            peer_repo,
            session_repo,
            connection_repo,
        }
    }

    /// Handle an incoming signaling message
    pub async fn handle_message(&self, message: SignalingMessage) -> Result<SignalingResponse> {
        // Validate message
        message.validate().map_err(|e| Error::InvalidInput(e))?;

        // Route to appropriate handler
        match message {
            SignalingMessage::Register { peer_id, public_addr, nat_type, capabilities: _ } => {
                let peer_capabilities = crate::domain::PeerCapabilities::new(nat_type, "1.0.0".to_string());
                let mut peer = Peer::new(PeerId::from(peer_id), peer_capabilities);
                peer.set_signaling_address(public_addr);
                peer.connect();
                self.peer_repo.save(&peer).await?;
                
                Ok(SignalingResponse::Registered {
                    peer_id,
                    server_time: Utc::now().timestamp(),
                })
            }
            SignalingMessage::Unregister { peer_id } => {
                self.peer_repo.delete(&PeerId::from(peer_id)).await?;
                Ok(SignalingResponse::Unregistered { peer_id })
            }
            SignalingMessage::InitiateSession { initiator_id, responder_id } => {
                let session = Session::new(PeerId::from(initiator_id), PeerId::from(responder_id));
                let session_id = *session.id().as_uuid();
                self.session_repo.save(&session).await?;
                
                Ok(SignalingResponse::SessionCreated {
                    session_id,
                    initiator_id,
                    responder_id,
                })
            }
            SignalingMessage::AcceptSession { session_id, responder_id } => {
                let mut session = self.session_repo.find_by_id(&SessionId::from(session_id)).await?
                    .ok_or(Error::NotFound(format!("Session {}", session_id)))?;
                
                // Start gathering candidates (accept the session)
                session.start_gathering();
                self.session_repo.save(&session).await?;
                
                let responder = self.peer_repo.find_by_id(&PeerId::from(responder_id)).await?
                    .ok_or(Error::NotFound(format!("Peer {}", responder_id)))?;
                
                let responder_addr = responder.signaling_address()
                    .ok_or(Error::InvalidState("Responder has no signaling address".to_string()))?;
                let responder_nat = responder.capabilities().nat_type();
                
                Ok(SignalingResponse::SessionAccepted {
                    session_id,
                    responder_id,
                    responder_addr,
                    responder_nat,
                })
            }
            SignalingMessage::RejectSession { session_id, reason, .. } => {
                let mut session = self.session_repo.find_by_id(&SessionId::from(session_id)).await?
                    .ok_or(Error::NotFound(format!("Session {}", session_id)))?;
                
                // Mark session as failed (reject it)
                session.fail(reason.clone());
                self.session_repo.save(&session).await?;
                
                Ok(SignalingResponse::SessionRejected { session_id, reason })
            }
            SignalingMessage::InitiateConnection { session_id, local_peer_id, remote_peer_id } => {
                let local_peer = self.peer_repo.find_by_id(&PeerId::from(local_peer_id)).await?
                    .ok_or(Error::NotFound(format!("Peer {}", local_peer_id)))?;
                let remote_peer = self.peer_repo.find_by_id(&PeerId::from(remote_peer_id)).await?
                    .ok_or(Error::NotFound(format!("Peer {}", remote_peer_id)))?;
                
                let connection = Connection::new(
                    SessionId::from(session_id),
                    PeerId::from(local_peer_id),
                    PeerId::from(remote_peer_id),
                    local_peer.capabilities().nat_type(),
                    remote_peer.capabilities().nat_type(),
                );
                
                let recommended_methods = connection.strategy().attempt_order();
                self.connection_repo.save(&connection).await?;
                
                // Use session_id as connection identifier since Connection doesn't have separate ID
                Ok(SignalingResponse::ConnectionInitiated {
                    connection_id: session_id,
                    session_id,
                    recommended_methods,
                })
            }
            SignalingMessage::UpdateConnection { connection_id, connection_method, local_addr: _, remote_addr: _ } => {
                // For now, return a simple update response
                // Full implementation would update connection state
                Ok(SignalingResponse::ConnectionUpdated {
                    connection_id,
                    connection_method,
                    status: "Attempting".to_string(),
                })
            }
            SignalingMessage::Heartbeat { peer_id } => {
                let mut peer = self.peer_repo.find_by_id(&PeerId::from(peer_id)).await?
                    .ok_or(Error::NotFound(format!("Peer {}", peer_id)))?;
                
                peer.update_activity();
                self.peer_repo.save(&peer).await?;
                
                Ok(SignalingResponse::HeartbeatAck {
                    peer_id,
                    server_time: Utc::now().timestamp(),
                })
            }
            SignalingMessage::QueryPeer { peer_id } => {
                let peer = self.peer_repo.find_by_id(&PeerId::from(peer_id)).await?
                    .ok_or(Error::NotFound(format!("Peer {}", peer_id)))?;
                
                let public_addr = peer.signaling_address()
                    .ok_or(Error::InvalidState("Peer has no signaling address".to_string()))?;
                
                Ok(SignalingResponse::PeerInfo {
                    peer_id,
                    public_addr,
                    nat_type: peer.capabilities().nat_type(),
                    is_online: peer.state() == crate::domain::PeerState::Connected,
                    capabilities: vec![],
                })
            }
            SignalingMessage::QuerySession { session_id } => {
                let session = self.session_repo.find_by_id(&SessionId::from(session_id)).await?
                    .ok_or(Error::NotFound(format!("Session {}", session_id)))?;
                
                Ok(SignalingResponse::SessionInfo {
                    session_id,
                    initiator_id: *session.initiator_id().as_uuid(),
                    responder_id: *session.responder_id().as_uuid(),
                    status: format!("{:?}", session.state()),
                    created_at: session.created_at().duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default().as_secs() as i64,
                })
            }
            SignalingMessage::QueryConnection { connection_id: _ } => {
                // Simplified - would need to find by connection ID
                Err(Error::NotFound(format!("Connection query not fully implemented")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::{
        InMemoryPeerRepository, InMemorySessionRepository, InMemoryConnectionRepository,
    };
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn create_test_handler() -> MessageHandler {
        let peer_repo = Arc::new(InMemoryPeerRepository::new());
        let session_repo = Arc::new(InMemorySessionRepository::new());
        let connection_repo = Arc::new(InMemoryConnectionRepository::new());

        MessageHandler::new(peer_repo, session_repo, connection_repo)
    }

    #[tokio::test]
    async fn test_handle_register() {
        let handler = create_test_handler();
        let peer_id = uuid::Uuid::new_v4();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);

        let message = SignalingMessage::Register {
            peer_id,
            public_addr: addr,
            nat_type: NatType::FullCone,
            capabilities: vec!["quic".to_string()],
        };

        let response = handler.handle_message(message).await.unwrap();

        match response {
            SignalingResponse::Registered { peer_id: resp_id, .. } => {
                assert_eq!(resp_id, peer_id);
            }
            _ => panic!("Expected Registered response"),
        }
    }

    #[tokio::test]
    async fn test_handle_unregister() {
        let handler = create_test_handler();
        let peer_id = uuid::Uuid::new_v4();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);

        // Register first
        let register_msg = SignalingMessage::Register {
            peer_id,
            public_addr: addr,
            nat_type: NatType::FullCone,
            capabilities: vec![],
        };
        handler.handle_message(register_msg).await.unwrap();

        // Then unregister
        let unregister_msg = SignalingMessage::Unregister { peer_id };
        let response = handler.handle_message(unregister_msg).await.unwrap();

        match response {
            SignalingResponse::Unregistered { peer_id: resp_id } => {
                assert_eq!(resp_id, peer_id);
            }
            _ => panic!("Expected Unregistered response"),
        }
    }

    #[tokio::test]
    async fn test_handle_heartbeat() {
        let handler = create_test_handler();
        let peer_id = uuid::Uuid::new_v4();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);

        // Register first
        handler.handle_message(SignalingMessage::Register {
            peer_id,
            public_addr: addr,
            nat_type: NatType::FullCone,
            capabilities: vec![],
        }).await.unwrap();

        // Send heartbeat
        let message = SignalingMessage::Heartbeat { peer_id };
        let response = handler.handle_message(message).await.unwrap();

        match response {
            SignalingResponse::HeartbeatAck { peer_id: resp_id, .. } => {
                assert_eq!(resp_id, peer_id);
            }
            _ => panic!("Expected HeartbeatAck response"),
        }
    }

    #[tokio::test]
    async fn test_invalid_message_validation() {
        let handler = create_test_handler();
        let peer_id = uuid::Uuid::new_v4();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);

        // Too many capabilities
        let message = SignalingMessage::Register {
            peer_id,
            public_addr: addr,
            nat_type: NatType::FullCone,
            capabilities: (0..101).map(|i| format!("cap{}", i)).collect(),
        };

        let result = handler.handle_message(message).await;
        assert!(result.is_err());
    }
}

// Made with Bob
