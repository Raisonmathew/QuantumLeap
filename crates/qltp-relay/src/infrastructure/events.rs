//! Event Publisher Implementations

use crate::error::Result;
use crate::ports::{DomainEvent, EventPublisher};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info};

/// In-memory event publisher using channels
#[derive(Clone)]
pub struct InMemoryEventPublisher {
    sender: mpsc::UnboundedSender<DomainEvent>,
}

impl InMemoryEventPublisher {
    /// Create a new in-memory event publisher
    pub fn new() -> (Self, mpsc::UnboundedReceiver<DomainEvent>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }

    /// Create a publisher that logs events but doesn't store them
    pub fn logging_only() -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel::<DomainEvent>();
        
        // Spawn a task to consume and log events
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                info!("Event: {} at {:?}", event.name(), event.timestamp());
                debug!("Event details: {:?}", event);
            }
        });

        Self { sender }
    }
}

impl Default for InMemoryEventPublisher {
    fn default() -> Self {
        Self::logging_only()
    }
}

#[async_trait]
impl EventPublisher for InMemoryEventPublisher {
    async fn publish(&self, event: DomainEvent) -> Result<()> {
        self.sender
            .send(event)
            .map_err(|e| crate::error::Error::Internal(format!("Failed to publish event: {}", e)))?;
        Ok(())
    }
}

/// Event subscriber for testing
pub struct EventSubscriber {
    receiver: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<DomainEvent>>>,
}

impl EventSubscriber {
    /// Create a new event subscriber
    pub fn new(receiver: mpsc::UnboundedReceiver<DomainEvent>) -> Self {
        Self {
            receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
        }
    }

    /// Receive the next event (blocking)
    pub async fn recv(&self) -> Option<DomainEvent> {
        self.receiver.lock().await.recv().await
    }

    /// Try to receive an event (non-blocking)
    pub async fn try_recv(&self) -> Option<DomainEvent> {
        self.receiver.lock().await.try_recv().ok()
    }

    /// Receive all pending events
    pub async fn recv_all(&self) -> Vec<DomainEvent> {
        let mut events = Vec::new();
        let mut receiver = self.receiver.lock().await;
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{PeerId, SessionId};
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_event_publisher_publish() {
        let (publisher, receiver) = InMemoryEventPublisher::new();
        let subscriber = EventSubscriber::new(receiver);

        let event = DomainEvent::PeerRegistered {
            peer_id: PeerId::new(),
            timestamp: SystemTime::now(),
        };

        publisher.publish(event.clone()).await.unwrap();

        let received = subscriber.recv().await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().name(), "peer.registered");
    }

    #[tokio::test]
    async fn test_event_publisher_batch() {
        let (publisher, receiver) = InMemoryEventPublisher::new();
        let subscriber = EventSubscriber::new(receiver);

        let events = vec![
            DomainEvent::PeerRegistered {
                peer_id: PeerId::new(),
                timestamp: SystemTime::now(),
            },
            DomainEvent::PeerConnected {
                peer_id: PeerId::new(),
                timestamp: SystemTime::now(),
            },
        ];

        publisher.publish_batch(events).await.unwrap();

        let received = subscriber.recv_all().await;
        assert_eq!(received.len(), 2);
    }

    #[tokio::test]
    async fn test_event_subscriber_try_recv() {
        let (publisher, receiver) = InMemoryEventPublisher::new();
        let subscriber = EventSubscriber::new(receiver);

        // No events yet
        assert!(subscriber.try_recv().await.is_none());

        // Publish event
        let event = DomainEvent::SessionCreated {
            session_id: SessionId::new(),
            initiator_id: PeerId::new(),
            responder_id: PeerId::new(),
            timestamp: SystemTime::now(),
        };
        publisher.publish(event).await.unwrap();

        // Should receive event
        assert!(subscriber.try_recv().await.is_some());

        // No more events
        assert!(subscriber.try_recv().await.is_none());
    }

    #[tokio::test]
    async fn test_logging_only_publisher() {
        let publisher = InMemoryEventPublisher::logging_only();

        let event = DomainEvent::PeerRegistered {
            peer_id: PeerId::new(),
            timestamp: SystemTime::now(),
        };

        // Should not fail even though events are just logged
        publisher.publish(event).await.unwrap();
    }
}

// Made with Bob
