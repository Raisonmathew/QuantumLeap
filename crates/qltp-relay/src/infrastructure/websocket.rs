//! WebSocket Server Infrastructure
//!
//! Provides WebSocket server for peer signaling and coordination.

use crate::error::{Error, Result};
use crate::infrastructure::config::WebSocketConfig;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info};

/// WebSocket connection ID
pub type ConnectionId = uuid::Uuid;

/// WebSocket server
pub struct WebSocketServer {
    config: WebSocketConfig,
    connections: Arc<RwLock<std::collections::HashMap<ConnectionId, WebSocketConnection>>>,
}

/// WebSocket connection wrapper
struct WebSocketConnection {
    id: ConnectionId,
    addr: SocketAddr,
    stream: WebSocketStream<TcpStream>,
}

impl WebSocketServer {
    /// Create a new WebSocket server
    pub fn new(config: WebSocketConfig) -> Self {
        Self {
            config,
            connections: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Start the WebSocket server
    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.bind_addr)
            .await
            .map_err(|e| Error::Internal(format!("Failed to bind WebSocket server: {}", e)))?;

        info!("WebSocket server listening on {}", self.config.bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New connection from {}", addr);
                    let connections = self.connections.clone();
                    let config = self.config.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, addr, connections, config).await {
                            error!("Connection error from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle a WebSocket connection
    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        connections: Arc<RwLock<std::collections::HashMap<ConnectionId, WebSocketConnection>>>,
        _config: WebSocketConfig,
    ) -> Result<()> {
        let ws_stream = accept_async(stream)
            .await
            .map_err(|e| Error::Internal(format!("WebSocket handshake failed: {}", e)))?;

        let conn_id = ConnectionId::new_v4();
        
        // Create connection wrapper
        let connection = WebSocketConnection {
            id: conn_id,
            addr,
            stream: ws_stream,
        };
        
        info!("WebSocket connection established: {} from {}", connection.id, connection.addr);

        // Store connection
        {
            let mut conns = connections.write().await;
            conns.insert(conn_id, connection);
        }

        // Retrieve connection for processing
        let connection = {
            let mut conns = connections.write().await;
            conns.remove(&conn_id).unwrap()
        };

        let (mut write, mut read) = connection.stream.split();

        // Send welcome message
        let welcome = serde_json::json!({
            "type": "welcome",
            "connection_id": conn_id.to_string(),
            "server_version": "1.0.0"
        });
        write
            .send(Message::Text(welcome.to_string()))
            .await
            .map_err(|e| Error::Internal(format!("Failed to send welcome: {}", e)))?;

        // Handle messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    debug!("Received text message from {}: {}", conn_id, text);
                    // Message handling will be implemented in adapters layer
                }
                Ok(Message::Binary(data)) => {
                    debug!("Received binary message from {}: {} bytes", conn_id, data.len());
                }
                Ok(Message::Ping(data)) => {
                    debug!("Received ping from {}", conn_id);
                    write
                        .send(Message::Pong(data))
                        .await
                        .map_err(|e| Error::Internal(format!("Failed to send pong: {}", e)))?;
                }
                Ok(Message::Pong(_)) => {
                    debug!("Received pong from {}", conn_id);
                }
                Ok(Message::Close(frame)) => {
                    info!("Connection closed by client {}: {:?}", conn_id, frame);
                    break;
                }
                Ok(Message::Frame(_)) => {
                    // Raw frames are handled internally
                }
                Err(e) => {
                    error!("WebSocket error from {}: {}", conn_id, e);
                    break;
                }
            }
        }

        // Cleanup
        connections.write().await.remove(&conn_id);
        info!("Connection {} closed", conn_id);

        Ok(())
    }

    /// Get number of active connections
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Broadcast message to all connections
    pub async fn broadcast(&self, _message: &str) -> Result<()> {
        let connections = self.connections.read().await;
        for (id, _conn) in connections.iter() {
            debug!("Broadcasting to connection {}", id);
            // Actual sending will be implemented when we have proper connection management
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_config() {
        let config = WebSocketConfig::default();
        assert_eq!(config.max_message_size, 1024 * 1024);
    }

    #[test]
    fn test_connection_id_generation() {
        let id1 = ConnectionId::new_v4();
        let id2 = ConnectionId::new_v4();
        assert_ne!(id1, id2);
    }

    #[tokio::test]
    async fn test_websocket_server_creation() {
        let config = WebSocketConfig::default();
        let server = WebSocketServer::new(config);
        assert_eq!(server.connection_count().await, 0);
    }
}

// Made with Bob
