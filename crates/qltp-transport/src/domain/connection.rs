//! Transport connection abstraction for file transfers

use crate::error::Result;
use async_trait::async_trait;

/// Transport connection interface for file transfers
///
/// This trait provides a simple send/receive interface that can be implemented
/// by different transport backends (TCP, QUIC, io_uring, etc.)
#[async_trait]
pub trait TransportConnection: Send + Sync {
    /// Send data over the connection
    ///
    /// # Arguments
    /// * `data` - The data to send
    ///
    /// # Returns
    /// The number of bytes sent
    async fn send(&mut self, data: &[u8]) -> Result<usize>;

    /// Receive data from the connection
    ///
    /// # Arguments
    /// * `buffer` - The buffer to receive data into
    ///
    /// # Returns
    /// The number of bytes received
    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize>;

    /// Close the connection
    async fn close(&mut self) -> Result<()>;

    /// Check if the connection is still open
    fn is_open(&self) -> bool;
}

// Made with Bob
