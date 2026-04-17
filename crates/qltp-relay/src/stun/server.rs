//! STUN Server Implementation
//!
//! Async STUN server using tokio for handling Binding Requests

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::{debug, error, info};

use super::attributes::{MappedAddress, StunAttribute};
use super::message::{StunMessage, StunMessageType};

/// STUN Server configuration
#[derive(Debug, Clone)]
pub struct StunServerConfig {
    /// Bind address
    pub bind_addr: SocketAddr,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Enable software attribute in responses
    pub include_software: bool,
    /// Software name
    pub software_name: String,
}

impl Default for StunServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:3478".parse().unwrap(),
            max_connections: 10000,
            include_software: true,
            software_name: "QLTP-STUN/1.0".to_string(),
        }
    }
}

/// STUN Server
pub struct StunServer {
    config: Arc<StunServerConfig>,
    socket: Arc<UdpSocket>,
}

impl StunServer {
    /// Create new STUN server
    pub async fn new(config: StunServerConfig) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(config.bind_addr).await?;
        info!("STUN server listening on {}", config.bind_addr);

        Ok(Self {
            config: Arc::new(config),
            socket: Arc::new(socket),
        })
    }

    /// Run the STUN server
    pub async fn run(&self) -> std::io::Result<()> {
        let mut buf = vec![0u8; 1500]; // MTU size

        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((len, peer_addr)) => {
                    let data = bytes::Bytes::copy_from_slice(&buf[..len]);
                    let socket = self.socket.clone();
                    let config = self.config.clone();

                    // Spawn task to handle request
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_request(data, peer_addr, socket, config).await {
                            error!("Error handling STUN request from {}: {}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error receiving UDP packet: {}", e);
                }
            }
        }
    }

    /// Handle a STUN request
    async fn handle_request(
        data: bytes::Bytes,
        peer_addr: SocketAddr,
        socket: Arc<UdpSocket>,
        config: Arc<StunServerConfig>,
    ) -> Result<(), String> {
        // Decode STUN message
        let request = StunMessage::decode(data)?;

        debug!(
            "Received STUN {} from {}",
            request.message_type, peer_addr
        );

        // Only handle Binding Requests
        if request.message_type != StunMessageType::binding_request() {
            return Err(format!("Unsupported message type: {}", request.message_type));
        }

        // Create Binding Response
        let mut response = StunMessage::binding_response(request.transaction_id);

        // Add XOR-MAPPED-ADDRESS attribute (peer's public address)
        response.add_attribute(StunAttribute::XorMappedAddress(MappedAddress::new(
            peer_addr,
        )));

        // Add SOFTWARE attribute if enabled
        if config.include_software {
            response.add_attribute(StunAttribute::Software(config.software_name.clone()));
        }

        // Encode and send response
        let response_data = response.encode();
        socket
            .send_to(&response_data, peer_addr)
            .await
            .map_err(|e| format!("Failed to send response: {}", e))?;

        debug!("Sent STUN response to {}", peer_addr);

        Ok(())
    }

    /// Get local address
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stun_server_creation() {
        let config = StunServerConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(), // Use random port
            ..Default::default()
        };

        let server = StunServer::new(config).await.unwrap();
        assert!(server.local_addr().is_ok());
    }
}

// Made with Bob
