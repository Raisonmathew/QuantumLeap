//! Event Publisher Interfaces
//!
//! These traits define the event notification interfaces that the application
//! layer uses to publish domain events. This enables loose coupling and
//! supports event-driven architecture.

use crate::domain::{ConnectionMethod, IceCandidate, PeerId, SessionId};
use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Domain event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DomainEvent {
    // Peer events
    PeerRegistered {
        peer_id: PeerId,
        timestamp: SystemTime,
    },
    PeerConnected {
        peer_id: PeerId,
        timestamp: SystemTime,
    },
    PeerDisconnected {
        peer_id: PeerId,
        reason: String,
        timestamp: SystemTime,
    },
    PeerFailed {
        peer_id: PeerId,
        error: String,
        timestamp: SystemTime,
    },
    IceCandidateAdded {
        peer_id: PeerId,
        candidate: IceCandidate,
        timestamp: SystemTime,
    },

    // Session events
    SessionCreated {
        session_id: SessionId,
        initiator_id: PeerId,
        responder_id: PeerId,
        timestamp: SystemTime,
    },
    SessionGathering {
        session_id: SessionId,
        timestamp: SystemTime,
    },
    SessionNegotiating {
        session_id: SessionId,
        timestamp: SystemTime,
    },
    SessionEstablished {
        session_id: SessionId,
        duration_ms: u64,
        timestamp: SystemTime,
    },
    SessionFailed {
        session_id: SessionId,
        error: String,
        timestamp: SystemTime,
    },
    SessionClosed {
        session_id: SessionId,
        timestamp: SystemTime,
    },

    // Connection events
    ConnectionInitiated {
        session_id: SessionId,
        strategy: String,
        timestamp: SystemTime,
    },
    ConnectionAttempting {
        session_id: SessionId,
        method: ConnectionMethod,
        timestamp: SystemTime,
    },
    ConnectionEstablished {
        session_id: SessionId,
        method: ConnectionMethod,
        duration_ms: u64,
        timestamp: SystemTime,
    },
    ConnectionFailed {
        session_id: SessionId,
        method: ConnectionMethod,
        error: String,
        timestamp: SystemTime,
    },
    ConnectionClosed {
        session_id: SessionId,
        timestamp: SystemTime,
    },
}

impl DomainEvent {
    /// Get event timestamp
    pub fn timestamp(&self) -> SystemTime {
        match self {
            DomainEvent::PeerRegistered { timestamp, .. }
            | DomainEvent::PeerConnected { timestamp, .. }
            | DomainEvent::PeerDisconnected { timestamp, .. }
            | DomainEvent::PeerFailed { timestamp, .. }
            | DomainEvent::IceCandidateAdded { timestamp, .. }
            | DomainEvent::SessionCreated { timestamp, .. }
            | DomainEvent::SessionGathering { timestamp, .. }
            | DomainEvent::SessionNegotiating { timestamp, .. }
            | DomainEvent::SessionEstablished { timestamp, .. }
            | DomainEvent::SessionFailed { timestamp, .. }
            | DomainEvent::SessionClosed { timestamp, .. }
            | DomainEvent::ConnectionInitiated { timestamp, .. }
            | DomainEvent::ConnectionAttempting { timestamp, .. }
            | DomainEvent::ConnectionEstablished { timestamp, .. }
            | DomainEvent::ConnectionFailed { timestamp, .. }
            | DomainEvent::ConnectionClosed { timestamp, .. } => *timestamp,
        }
    }

    /// Get event name
    pub fn name(&self) -> &'static str {
        match self {
            DomainEvent::PeerRegistered { .. } => "peer.registered",
            DomainEvent::PeerConnected { .. } => "peer.connected",
            DomainEvent::PeerDisconnected { .. } => "peer.disconnected",
            DomainEvent::PeerFailed { .. } => "peer.failed",
            DomainEvent::IceCandidateAdded { .. } => "peer.ice_candidate_added",
            DomainEvent::SessionCreated { .. } => "session.created",
            DomainEvent::SessionGathering { .. } => "session.gathering",
            DomainEvent::SessionNegotiating { .. } => "session.negotiating",
            DomainEvent::SessionEstablished { .. } => "session.established",
            DomainEvent::SessionFailed { .. } => "session.failed",
            DomainEvent::SessionClosed { .. } => "session.closed",
            DomainEvent::ConnectionInitiated { .. } => "connection.initiated",
            DomainEvent::ConnectionAttempting { .. } => "connection.attempting",
            DomainEvent::ConnectionEstablished { .. } => "connection.established",
            DomainEvent::ConnectionFailed { .. } => "connection.failed",
            DomainEvent::ConnectionClosed { .. } => "connection.closed",
        }
    }

    /// Check if event is a peer event
    pub fn is_peer_event(&self) -> bool {
        matches!(
            self,
            DomainEvent::PeerRegistered { .. }
                | DomainEvent::PeerConnected { .. }
                | DomainEvent::PeerDisconnected { .. }
                | DomainEvent::PeerFailed { .. }
                | DomainEvent::IceCandidateAdded { .. }
        )
    }

    /// Check if event is a session event
    pub fn is_session_event(&self) -> bool {
        matches!(
            self,
            DomainEvent::SessionCreated { .. }
                | DomainEvent::SessionGathering { .. }
                | DomainEvent::SessionNegotiating { .. }
                | DomainEvent::SessionEstablished { .. }
                | DomainEvent::SessionFailed { .. }
                | DomainEvent::SessionClosed { .. }
        )
    }

    /// Check if event is a connection event
    pub fn is_connection_event(&self) -> bool {
        matches!(
            self,
            DomainEvent::ConnectionInitiated { .. }
                | DomainEvent::ConnectionAttempting { .. }
                | DomainEvent::ConnectionEstablished { .. }
                | DomainEvent::ConnectionFailed { .. }
                | DomainEvent::ConnectionClosed { .. }
        )
    }
}

/// Event publisher trait
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Publish a domain event
    async fn publish(&self, event: DomainEvent) -> Result<()>;

    /// Publish multiple events
    async fn publish_batch(&self, events: Vec<DomainEvent>) -> Result<()> {
        for event in events {
            self.publish(event).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // Mock event publisher for testing
    struct MockEventPublisher {
        events: Arc<Mutex<Vec<DomainEvent>>>,
    }

    impl MockEventPublisher {
        fn new() -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_events(&self) -> Vec<DomainEvent> {
            self.events.lock().unwrap().clone()
        }

        fn clear(&self) {
            self.events.lock().unwrap().clear();
        }
    }

    #[async_trait]
    impl EventPublisher for MockEventPublisher {
        async fn publish(&self, event: DomainEvent) -> Result<()> {
            self.events.lock().unwrap().push(event);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_event_publisher_publish() {
        let publisher = MockEventPublisher::new();
        let peer_id = PeerId::new();

        let event = DomainEvent::PeerRegistered {
            peer_id: peer_id.clone(),
            timestamp: SystemTime::now(),
        };

        publisher.publish(event.clone()).await.unwrap();

        let events = publisher.get_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name(), "peer.registered");
    }

    #[tokio::test]
    async fn test_event_publisher_batch() {
        let publisher = MockEventPublisher::new();
        let peer_id = PeerId::new();

        let events = vec![
            DomainEvent::PeerRegistered {
                peer_id: peer_id.clone(),
                timestamp: SystemTime::now(),
            },
            DomainEvent::PeerConnected {
                peer_id: peer_id.clone(),
                timestamp: SystemTime::now(),
            },
        ];

        publisher.publish_batch(events).await.unwrap();

        let published = publisher.get_events();
        assert_eq!(published.len(), 2);
    }

    #[test]
    fn test_event_name() {
        let peer_id = PeerId::new();
        let event = DomainEvent::PeerRegistered {
            peer_id,
            timestamp: SystemTime::now(),
        };

        assert_eq!(event.name(), "peer.registered");
    }

    #[test]
    fn test_event_type_checks() {
        let peer_id = PeerId::new();
        let session_id = SessionId::new();

        let peer_event = DomainEvent::PeerRegistered {
            peer_id,
            timestamp: SystemTime::now(),
        };
        assert!(peer_event.is_peer_event());
        assert!(!peer_event.is_session_event());
        assert!(!peer_event.is_connection_event());

        let session_event = DomainEvent::SessionCreated {
            session_id: session_id.clone(),
            initiator_id: PeerId::new(),
            responder_id: PeerId::new(),
            timestamp: SystemTime::now(),
        };
        assert!(!session_event.is_peer_event());
        assert!(session_event.is_session_event());
        assert!(!session_event.is_connection_event());

        let connection_event = DomainEvent::ConnectionInitiated {
            session_id,
            strategy: "DirectP2PFirst".to_string(),
            timestamp: SystemTime::now(),
        };
        assert!(!connection_event.is_peer_event());
        assert!(!connection_event.is_session_event());
        assert!(connection_event.is_connection_event());
    }
}

// Made with Bob
