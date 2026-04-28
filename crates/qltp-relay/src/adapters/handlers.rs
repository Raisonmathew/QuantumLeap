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

/// Maximum number of OCC retries before giving up and surfacing the conflict.
///
/// Chosen to absorb realistic concurrent contention on a single session
/// (typically 2-3 racers from duplicate signaling deliveries) while still
/// bounding the worst-case latency at O(MAX_OCC_RETRIES * single_op_latency).
const MAX_OCC_RETRIES: usize = 8;

/// Apply a mutation to a session under optimistic concurrency control.
///
/// This is the standard "load -> mutate -> CAS commit -> retry on conflict"
/// loop used by JPA `@Version`, DynamoDB conditional writes, Etcd revisions,
/// and similar OCC systems. The mutation closure runs against the latest
/// snapshot every retry, so even non-idempotent transitions (counters,
/// monotonic state machines) compose correctly.
///
/// Returns the persisted session on success. Returns `Error::Conflict` only
/// after exhausting `MAX_OCC_RETRIES`, which indicates pathological
/// contention rather than a normal duplicate-delivery race.
async fn update_session_with_retry<F>(
    repo: &dyn SessionRepository,
    id: &SessionId,
    mut mutate: F,
) -> Result<Session>
where
    F: FnMut(&mut Session) -> Result<()>,
{
    for attempt in 0..MAX_OCC_RETRIES {
        let mut session = repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Session {}", id.as_uuid())))?;
        let expected_version = session.version();
        mutate(&mut session)?;
        match repo.update_if_unchanged(&session, expected_version).await {
            Ok(()) => return Ok(session),
            Err(Error::Conflict(_)) if attempt + 1 < MAX_OCC_RETRIES => {
                // Lost the CAS race; reload and try again. No backoff needed:
                // the contended path is in-memory and short, and the producer
                // of the conflict has already committed its work.
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict(format!(
        "Session {} could not be updated after {} OCC retries",
        id.as_uuid(),
        MAX_OCC_RETRIES
    )))
}

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
                // Concurrency-safe state transition via OCC.
                //
                // Two simultaneous `AcceptSession` messages for the same
                // session_id used to race here (load -> mutate -> save with no
                // serialization point). We now load the session, mutate locally,
                // and commit through `update_if_unchanged`, which is an atomic
                // compare-and-swap at the repository layer. On version
                // mismatch we reload and retry up to MAX_OCC_RETRIES times.
                update_session_with_retry(
                    self.session_repo.as_ref(),
                    &SessionId::from(session_id),
                    |s| { s.start_gathering(); Ok(()) },
                ).await?;

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
                // Same OCC pattern as AcceptSession - guards against duplicate
                // RejectSession messages racing with each other or with an
                // AcceptSession that arrives in the same tick.
                let reason_for_mut = reason.clone();
                update_session_with_retry(
                    self.session_repo.as_ref(),
                    &SessionId::from(session_id),
                    move |s| { s.fail(reason_for_mut.clone()); Ok(()) },
                ).await?;

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
    use crate::domain::nat_type::NatType;
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

    /// Stress test: prove that under heavy concurrent contention on the same
    /// session id, every increment is observed (no lost updates) and the
    /// final OCC version matches the number of successful commits. This is
    /// the property the doc-only fix could not give us.
    #[tokio::test]
    async fn test_occ_no_lost_updates_under_concurrency() {
        use crate::domain::{Peer, PeerCapabilities, Session};
        use tokio::task::JoinSet;

        let session_repo: Arc<dyn SessionRepository> =
            Arc::new(InMemorySessionRepository::new());

        // Seed a session.
        let initiator = PeerId::new();
        let responder = PeerId::new();
        let session = Session::new(initiator, responder);
        let session_id = session.id().clone();
        let starting_version = session.version();
        session_repo.save(&session).await.unwrap();

        // 32 concurrent writers each apply a non-idempotent state mutation
        // (incrementing connection_attempts via `increment_attempts`).
        const N_WRITERS: u32 = 32;
        let mut set = JoinSet::new();
        for _ in 0..N_WRITERS {
            let repo = session_repo.clone();
            let id = session_id.clone();
            set.spawn(async move {
                update_session_with_retry(repo.as_ref(), &id, |s| {
                    s.increment_attempts();
                    Ok(())
                })
                .await
            });
        }

        let mut successes = 0u32;
        while let Some(res) = set.join_next().await {
            res.unwrap().expect("OCC retry must eventually succeed under bounded contention");
            successes += 1;
        }
        assert_eq!(successes, N_WRITERS);

        // Every successful commit must be reflected: connection_attempts and
        // the OCC version both advanced by exactly N_WRITERS.
        let final_session = session_repo
            .find_by_id(&session_id)
            .await
            .unwrap()
            .expect("session must still exist");
        assert_eq!(
            final_session.connection_attempts(),
            N_WRITERS,
            "non-idempotent mutation must compose exactly once per writer"
        );
        assert_eq!(
            final_session.version(),
            starting_version + N_WRITERS as u64,
            "OCC version must advance once per successful CAS"
        );

        // Silence unused warnings for repo types only used by this test path.
        let _ = (Peer::new(PeerId::new(), PeerCapabilities::new(NatType::FullCone, "1.0".into())),);
    }

    /// Direct CAS test: a stale write (wrong expected_version) must be
    /// rejected with `Error::Conflict` and must not mutate state.
    #[tokio::test]
    async fn test_update_if_unchanged_rejects_stale_write() {
        use crate::domain::Session;

        let repo: Arc<dyn SessionRepository> = Arc::new(InMemorySessionRepository::new());
        let mut session = Session::new(PeerId::new(), PeerId::new());
        repo.save(&session).await.unwrap();
        let v0 = session.version();

        // First writer commits at v0.
        let mut writer_a = session.clone();
        writer_a.increment_attempts();
        repo.update_if_unchanged(&writer_a, v0).await.expect("first writer wins");

        // Second writer also held v0; commit must be rejected.
        session.increment_attempts();
        let err = repo
            .update_if_unchanged(&session, v0)
            .await
            .expect_err("stale write must be rejected");
        assert!(matches!(err, Error::Conflict(_)), "got {err:?}");

        // Stored state must still be writer_a's commit, not the stale write.
        let stored = repo.find_by_id(session.id()).await.unwrap().unwrap();
        assert_eq!(stored.connection_attempts(), 1);
    }
}

// Made with Bob
