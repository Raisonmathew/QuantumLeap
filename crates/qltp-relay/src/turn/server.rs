//! TURN Server Implementation
//!
//! Handles TURN protocol messages and manages relay functionality

use bytes::Bytes;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time;

use crate::stun::{StunMessage, StunMessageType, StunMethod, StunClass, StunAttribute};
use super::allocation::{AllocationManager, TransportProtocol};
use super::attributes::{TurnAttribute, TurnAttributeType};

/// TURN Server Configuration
#[derive(Debug, Clone)]
pub struct TurnServerConfig {
    /// Server bind address
    pub bind_addr: SocketAddr,
    /// Relay address base (for allocating relay addresses)
    pub relay_base_addr: SocketAddr,
    /// Maximum concurrent allocations
    pub max_allocations: usize,
    /// Default allocation lifetime (seconds)
    pub default_lifetime: u32,
    /// Cleanup interval for expired allocations
    pub cleanup_interval: Duration,
    /// Server software name
    pub software: String,
}

impl Default for TurnServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:3478".parse().unwrap(),
            relay_base_addr: "0.0.0.0:0".parse().unwrap(),
            max_allocations: 1000,
            default_lifetime: 600,
            cleanup_interval: Duration::from_secs(60),
            software: "QLTP-TURN/1.0".to_string(),
        }
    }
}

/// TURN Server
pub struct TurnServer {
    config: TurnServerConfig,
    allocation_manager: Arc<AllocationManager>,
    socket: Arc<UdpSocket>,
}

impl TurnServer {
    /// Create new TURN server
    pub async fn new(config: TurnServerConfig) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind(config.bind_addr).await?;
        let allocation_manager = Arc::new(AllocationManager::new(config.relay_base_addr));

        Ok(Self {
            config,
            allocation_manager,
            socket: Arc::new(socket),
        })
    }

    /// Run the TURN server
    pub async fn run(self: Arc<Self>) -> Result<(), std::io::Error> {
        println!("TURN server listening on {}", self.config.bind_addr);

        // Start cleanup task
        let cleanup_manager = self.allocation_manager.clone();
        let cleanup_interval = self.config.cleanup_interval;
        tokio::spawn(async move {
            let mut interval = time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                cleanup_manager.cleanup_expired().await;
            }
        });

        // Main server loop
        let mut buf = vec![0u8; 65536];
        loop {
            let (len, addr) = self.socket.recv_from(&mut buf).await?;
            let data = Bytes::copy_from_slice(&buf[..len]);

            // Spawn task to handle request
            let server = self.clone();
            tokio::spawn(async move {
                if let Err(e) = server.handle_message(data, addr).await {
                    eprintln!("Error handling message from {}: {}", addr, e);
                }
            });
        }
    }

    /// Handle incoming TURN message
    async fn handle_message(&self, data: Bytes, client_addr: SocketAddr) -> Result<(), String> {
        // Decode STUN message
        let message = StunMessage::decode(data)?;

        // Handle based on method
        match message.message_type.method {
            StunMethod::Allocate => self.handle_allocate(message, client_addr).await,
            StunMethod::Refresh => self.handle_refresh(message, client_addr).await,
            StunMethod::CreatePermission => self.handle_create_permission(message, client_addr).await,
            StunMethod::ChannelBind => self.handle_channel_bind(message, client_addr).await,
            StunMethod::Send => self.handle_send(message, client_addr).await,
            _ => {
                // Unknown method, send error response
                self.send_error_response(message, client_addr, 400, "Bad Request").await
            }
        }
    }

    /// Handle Allocate request
    async fn handle_allocate(&self, request: StunMessage, client_addr: SocketAddr) -> Result<(), String> {
        // Check allocation limit
        if self.allocation_manager.allocation_count().await >= self.config.max_allocations {
            return self.send_error_response(request, client_addr, 486, "Allocation Quota Reached").await;
        }

        // Extract REQUESTED-TRANSPORT attribute
        let transport = request.attributes.iter()
            .find_map(|attr| {
                if let StunAttribute::Unknown { attr_type, value } = attr {
                    if *attr_type == TurnAttributeType::RequestedTransport as u16 {
                        if let Ok(turn_attr) = TurnAttribute::decode(*attr_type, value.clone(), &request.transaction_id) {
                            if let TurnAttribute::RequestedTransport(protocol) = turn_attr {
                                return Some(match protocol {
                                    super::attributes::TransportProtocol::Udp => TransportProtocol::Udp,
                                    super::attributes::TransportProtocol::Tcp => TransportProtocol::Tcp,
                                });
                            }
                        }
                    }
                }
                None
            })
            .ok_or_else(|| "REQUESTED-TRANSPORT attribute required".to_string())?;

        // Extract LIFETIME attribute (optional)
        let requested_lifetime = request.attributes.iter()
            .find_map(|attr| {
                if let StunAttribute::Unknown { attr_type, value } = attr {
                    if *attr_type == TurnAttributeType::Lifetime as u16 {
                        if let Ok(turn_attr) = TurnAttribute::decode(*attr_type, value.clone(), &request.transaction_id) {
                            if let TurnAttribute::Lifetime(lifetime) = turn_attr {
                                return Some(lifetime);
                            }
                        }
                    }
                }
                None
            })
            .unwrap_or(self.config.default_lifetime);

        // Create allocation
        let allocation = self.allocation_manager
            .create_allocation(
                client_addr,
                self.config.bind_addr,
                transport,
                requested_lifetime,
            )
            .await?;

        // Build success response
        let mut response = StunMessage::new(
            StunMessageType::new(StunClass::SuccessResponse, StunMethod::Allocate),
            request.transaction_id,
        );

        // Add XOR-RELAYED-ADDRESS
        let relayed_attr = TurnAttribute::XorRelayedAddress(allocation.relay_addr);
        response.attributes.push(StunAttribute::Unknown {
            attr_type: TurnAttributeType::XorRelayedAddress as u16,
            value: relayed_attr.encode(&request.transaction_id).slice(4..),
        });

        // Add LIFETIME
        let lifetime_attr = TurnAttribute::Lifetime(allocation.lifetime.as_secs() as u32);
        response.attributes.push(StunAttribute::Unknown {
            attr_type: TurnAttributeType::Lifetime as u16,
            value: lifetime_attr.encode(&request.transaction_id).slice(4..),
        });

        // Add SOFTWARE
        response.attributes.push(StunAttribute::Software(self.config.software.clone()));

        // Send response
        self.send_response(response, client_addr).await
    }

    /// Handle Refresh request
    async fn handle_refresh(&self, request: StunMessage, client_addr: SocketAddr) -> Result<(), String> {
        // Extract LIFETIME attribute
        let requested_lifetime = request.attributes.iter()
            .find_map(|attr| {
                if let StunAttribute::Unknown { attr_type, value } = attr {
                    if *attr_type == TurnAttributeType::Lifetime as u16 {
                        if let Ok(turn_attr) = TurnAttribute::decode(*attr_type, value.clone(), &request.transaction_id) {
                            if let TurnAttribute::Lifetime(lifetime) = turn_attr {
                                return Some(lifetime);
                            }
                        }
                    }
                }
                None
            })
            .unwrap_or(self.config.default_lifetime);

        // Refresh allocation
        let new_lifetime = self.allocation_manager
            .refresh_allocation(&client_addr, requested_lifetime)
            .await?;

        // Build success response
        let mut response = StunMessage::new(
            StunMessageType::new(StunClass::SuccessResponse, StunMethod::Refresh),
            request.transaction_id,
        );

        // Add LIFETIME
        let lifetime_attr = TurnAttribute::Lifetime(new_lifetime);
        response.attributes.push(StunAttribute::Unknown {
            attr_type: TurnAttributeType::Lifetime as u16,
            value: lifetime_attr.encode(&request.transaction_id).slice(4..),
        });

        // Send response
        self.send_response(response, client_addr).await
    }

    /// Handle CreatePermission request
    async fn handle_create_permission(&self, request: StunMessage, client_addr: SocketAddr) -> Result<(), String> {
        // Extract XOR-PEER-ADDRESS attributes
        let peer_addrs: Vec<SocketAddr> = request.attributes.iter()
            .filter_map(|attr| {
                if let StunAttribute::Unknown { attr_type, value } = attr {
                    if *attr_type == TurnAttributeType::XorPeerAddress as u16 {
                        if let Ok(turn_attr) = TurnAttribute::decode(*attr_type, value.clone(), &request.transaction_id) {
                            if let TurnAttribute::XorPeerAddress(addr) = turn_attr {
                                return Some(addr);
                            }
                        }
                    }
                }
                None
            })
            .collect();

        if peer_addrs.is_empty() {
            return self.send_error_response(request, client_addr, 400, "XOR-PEER-ADDRESS required").await;
        }

        // Add permissions (5 minute lifetime per RFC 5766)
        let permission_lifetime = Duration::from_secs(300);
        for peer_addr in peer_addrs {
            self.allocation_manager
                .add_permission(&client_addr, peer_addr, permission_lifetime)
                .await?;
        }

        // Build success response
        let response = StunMessage::new(
            StunMessageType::new(StunClass::SuccessResponse, StunMethod::CreatePermission),
            request.transaction_id,
        );

        // Send response
        self.send_response(response, client_addr).await
    }

    /// Handle ChannelBind request
    async fn handle_channel_bind(&self, request: StunMessage, client_addr: SocketAddr) -> Result<(), String> {
        // Extract CHANNEL-NUMBER
        let channel_number = request.attributes.iter()
            .find_map(|attr| {
                if let StunAttribute::Unknown { attr_type, value } = attr {
                    if *attr_type == TurnAttributeType::ChannelNumber as u16 {
                        if let Ok(turn_attr) = TurnAttribute::decode(*attr_type, value.clone(), &request.transaction_id) {
                            if let TurnAttribute::ChannelNumber(num) = turn_attr {
                                return Some(num);
                            }
                        }
                    }
                }
                None
            })
            .ok_or_else(|| "CHANNEL-NUMBER required".to_string())?;

        // Extract XOR-PEER-ADDRESS
        let peer_addr = request.attributes.iter()
            .find_map(|attr| {
                if let StunAttribute::Unknown { attr_type, value } = attr {
                    if *attr_type == TurnAttributeType::XorPeerAddress as u16 {
                        if let Ok(turn_attr) = TurnAttribute::decode(*attr_type, value.clone(), &request.transaction_id) {
                            if let TurnAttribute::XorPeerAddress(addr) = turn_attr {
                                return Some(addr);
                            }
                        }
                    }
                }
                None
            })
            .ok_or_else(|| "XOR-PEER-ADDRESS required".to_string())?;

        // Bind channel
        self.allocation_manager
            .bind_channel(&client_addr, channel_number, peer_addr)
            .await?;

        // Build success response
        let response = StunMessage::new(
            StunMessageType::new(StunClass::SuccessResponse, StunMethod::ChannelBind),
            request.transaction_id,
        );

        // Send response
        self.send_response(response, client_addr).await
    }

    /// Handle Send indication
    async fn handle_send(&self, request: StunMessage, client_addr: SocketAddr) -> Result<(), String> {
        // Extract XOR-PEER-ADDRESS
        let peer_addr = request.attributes.iter()
            .find_map(|attr| {
                if let StunAttribute::Unknown { attr_type, value } = attr {
                    if *attr_type == TurnAttributeType::XorPeerAddress as u16 {
                        if let Ok(turn_attr) = TurnAttribute::decode(*attr_type, value.clone(), &request.transaction_id) {
                            if let TurnAttribute::XorPeerAddress(addr) = turn_attr {
                                return Some(addr);
                            }
                        }
                    }
                }
                None
            })
            .ok_or_else(|| "XOR-PEER-ADDRESS required".to_string())?;

        // Extract DATA
        let data = request.attributes.iter()
            .find_map(|attr| {
                if let StunAttribute::Unknown { attr_type, value } = attr {
                    if *attr_type == TurnAttributeType::Data as u16 {
                        if let Ok(turn_attr) = TurnAttribute::decode(*attr_type, value.clone(), &request.transaction_id) {
                            if let TurnAttribute::Data(data) = turn_attr {
                                return Some(data);
                            }
                        }
                    }
                }
                None
            })
            .ok_or_else(|| "DATA required".to_string())?;

        // Get allocation
        let allocation = self.allocation_manager
            .get_by_client(&client_addr)
            .await
            .ok_or_else(|| "No allocation found".to_string())?;

        // Check permission
        if !allocation.has_permission(&peer_addr) {
            return Err("No permission for peer".to_string());
        }

        // Forward data to peer (simplified - in production, use relay socket)
        // This would be handled by the relay infrastructure
        println!("Relaying {} bytes from {} to {} via {}", 
                 data.len(), client_addr, peer_addr, allocation.relay_addr);

        Ok(())
    }

    /// Send success response
    async fn send_response(&self, response: StunMessage, addr: SocketAddr) -> Result<(), String> {
        let data = response.encode();
        self.socket.send_to(&data, addr).await
            .map_err(|e| format!("Failed to send response: {}", e))?;
        Ok(())
    }

    /// Send error response
    async fn send_error_response(
        &self,
        request: StunMessage,
        addr: SocketAddr,
        code: u16,
        reason: &str,
    ) -> Result<(), String> {
        let mut response = StunMessage::new(
            StunMessageType::new(StunClass::ErrorResponse, request.message_type.method),
            request.transaction_id,
        );

        response.attributes.push(StunAttribute::ErrorCode {
            code,
            reason: reason.to_string(),
        });

        self.send_response(response, addr).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_turn_server_creation() {
        let config = TurnServerConfig::default();
        let server = TurnServer::new(config).await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_allocation_manager() {
        let manager = AllocationManager::new("127.0.0.1:3478".parse().unwrap());
        let client_addr = "192.168.1.100:5000".parse().unwrap();
        let server_addr = "10.0.0.1:3478".parse().unwrap();

        let allocation = manager
            .create_allocation(client_addr, server_addr, TransportProtocol::Udp, 600)
            .await
            .unwrap();

        assert_eq!(allocation.client_addr, client_addr);
        assert_eq!(manager.allocation_count().await, 1);
    }
}

// Made with Bob