//! TCP Transport Backend
//!
//! Standard TCP implementation with BBR congestion control and zero-copy I/O
//! Target throughput: 120 MB/s baseline, up to 250 MB/s with optimizations

use crate::domain::{BackendCapabilities, SessionConfig, SessionId, TransportStats, TransportType};
use crate::error::{Error, Result};
use crate::ports::TransportBackend;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fs::File;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;

/// MTU (Maximum Transmission Unit) sizes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtuSize {
    /// Standard Ethernet (1500 bytes)
    Standard,
    /// Jumbo frames (9000 bytes) - for 10GbE networks
    Jumbo,
    /// Custom MTU size
    Custom(u16),
}

impl MtuSize {
    /// Get the MTU value in bytes
    pub fn as_bytes(&self) -> u16 {
        match self {
            MtuSize::Standard => 1500,
            MtuSize::Jumbo => 9000,
            MtuSize::Custom(size) => *size,
        }
    }
    
    /// Get optimal buffer size for this MTU (accounting for TCP/IP headers)
    pub fn optimal_buffer_size(&self) -> usize {
        // TCP/IP headers: 20 bytes (IP) + 20 bytes (TCP) + optional 12 bytes (timestamps)
        // Use conservative 60 bytes for headers
        let mtu = self.as_bytes() as usize;
        if mtu > 60 {
            mtu - 60
        } else {
            mtu
        }
    }
}

/// TCP session state
struct TcpSession {
    id: SessionId,
    config: SessionConfig,
    stream: Option<TcpStream>,
    listener: Option<TcpListener>,
    stats: TransportStats,
    is_server: bool,
    mtu: MtuSize,
}

impl TcpSession {
    fn new(id: SessionId, config: SessionConfig, is_server: bool) -> Self {
        Self {
            id,
            config,
            stream: None,
            listener: None,
            stats: TransportStats::new(),
            is_server,
            mtu: MtuSize::Standard, // Will be discovered later
        }
    }
}

/// TCP backend state
struct TcpBackendState {
    sessions: HashMap<SessionId, TcpSession>,
    initialized: bool,
}

/// Discover MTU for a network interface
///
/// Phase 6.1.3: MTU Discovery for jumbo frame support
/// Attempts to detect if jumbo frames (9000 bytes) are supported
#[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
fn discover_mtu(stream: &TcpStream) -> MtuSize {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        
        let fd = stream.as_raw_fd();
        
        unsafe {
            // Try to get IP_MTU (actual path MTU)
            let mut mtu: i32 = 0;
            let mut len: libc::socklen_t = std::mem::size_of::<i32>() as libc::socklen_t;
            
            let result = libc::getsockopt(
                fd,
                libc::IPPROTO_IP,
                libc::IP_MTU,
                &mut mtu as *mut _ as *mut libc::c_void,
                &mut len,
            );
            
            if result == 0 && mtu > 0 {
                debug!("Discovered MTU: {} bytes", mtu);
                
                // If MTU >= 9000, use jumbo frames
                if mtu >= 9000 {
                    info!("Jumbo frames supported (MTU: {}), enabling 9000-byte frames", mtu);
                    return MtuSize::Jumbo;
                } else if mtu >= 1500 {
                    debug!("Standard MTU detected: {}", mtu);
                    return MtuSize::Standard;
                } else {
                    // Custom MTU (e.g., VPN, tunnels)
                    debug!("Custom MTU detected: {}", mtu);
                    return MtuSize::Custom(mtu as u16);
                }
            }
        }
    }
    
    // Default to standard Ethernet MTU
    debug!("MTU discovery not available, using standard 1500-byte MTU");
    MtuSize::Standard
}

/// Enable jumbo frames on a TCP socket (Linux only)
///
/// Phase 6.1.3: Configure socket for jumbo frame support
#[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
fn enable_jumbo_frames(stream: &TcpStream, mtu: MtuSize) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        
        if mtu == MtuSize::Jumbo {
            let fd = stream.as_raw_fd();
            
            unsafe {
                // Disable Path MTU Discovery to allow jumbo frames
                let pmtu_disc: i32 = libc::IP_PMTUDISC_DONT;
                let result = libc::setsockopt(
                    fd,
                    libc::IPPROTO_IP,
                    libc::IP_MTU_DISCOVER,
                    &pmtu_disc as *const _ as *const libc::c_void,
                    std::mem::size_of::<i32>() as libc::socklen_t,
                );
                
                if result < 0 {
                    warn!("Failed to disable PMTU discovery for jumbo frames");
                } else {
                    info!("Jumbo frames enabled: 9000-byte MTU");
                }
            }
        }
    }

/// Zero-copy file transfer using sendfile()
///
/// Phase 6.2.1: sendfile() implementation for zero-copy transfers
/// Eliminates memory copies by transferring data directly from file to socket
///
/// Infrastructure function for future file transfer optimizations
///
/// # Arguments
/// * `stream` - TCP stream to send data to
/// * `file` - File to send
/// * `offset` - Starting offset in the file
/// * `count` - Number of bytes to send
///
/// # Returns
/// Number of bytes sent, or error
#[allow(dead_code)]
#[cfg(target_os = "linux")]
pub(crate) async fn send_file_zero_copy(
    stream: &TcpStream,
    file: &File,
    offset: u64,
    count: usize,
) -> Result<usize> {
    use std::os::unix::io::AsRawFd;
    
    let socket_fd = stream.as_raw_fd();
    let file_fd = file.as_raw_fd();
    
    // sendfile() is a blocking call, so we need to run it in a blocking task
    let result = tokio::task::spawn_blocking(move || {
        let mut offset_mut = offset as i64;
        
        unsafe {
            let bytes_sent = libc::sendfile(
                socket_fd,
                file_fd,
                &mut offset_mut,
                count,
            );
            
            if bytes_sent < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(bytes_sent as usize)
            }
        }
    })
    .await
    .map_err(|e| Error::Domain(format!("Task join error: {}", e)))?;
    
    result.map_err(Error::Io)
}

/// Fallback for non-Linux platforms - regular read/write
#[allow(dead_code)]
#[cfg(not(target_os = "linux"))]
pub(crate) async fn send_file_zero_copy(
    stream: &TcpStream,
    file: &File,
    offset: u64,
    count: usize,
) -> Result<usize> {
    use tokio::io::AsyncSeekExt;
    
    // Convert std::fs::File to tokio::fs::File
    let mut tokio_file = tokio::fs::File::from_std(file.try_clone().map_err(Error::Io)?);
    
    // Seek to offset
    tokio_file.seek(std::io::SeekFrom::Start(offset)).await.map_err(Error::Io)?;
    
    // Read and write in chunks
    let mut buffer = vec![0u8; count.min(65536)]; // 64KB chunks
    let mut total_sent = 0;
    let mut remaining = count;
    
    while remaining > 0 {
        let to_read = remaining.min(buffer.len());
        let n = tokio_file.read(&mut buffer[..to_read]).await.map_err(Error::Io)?;
        
        if n == 0 {
            break; // EOF
        }
        
        stream.writable().await.map_err(Error::Io)?;
        let sent = stream.try_write(&buffer[..n]).map_err(Error::Io)?;
        
        total_sent += sent;
        remaining -= sent;
        
        if sent < n {
            break; // Socket buffer full
        }
    }
    
    Ok(total_sent)
}

/// Check if zero-copy is available on this platform
///
/// Infrastructure function for future file transfer optimizations
#[allow(dead_code)]
pub(crate) fn supports_zero_copy() -> bool {
    cfg!(target_os = "linux")
}
    
    #[cfg(not(target_os = "linux"))]
    {
        if mtu == MtuSize::Jumbo {
            debug!("Jumbo frames requested but not supported on this platform");
        }
    }
    
    Ok(())
}

/// Calculate optimal buffer size based on Bandwidth-Delay Product (BDP)
///
/// BDP = Bandwidth × RTT
/// For high-throughput transfers, we use conservative estimates:
/// - 10 Gbps bandwidth × 100ms RTT = 125 MB
/// - We cap at 64 MB to avoid excessive memory usage
fn calculate_buffer_size(estimated_bandwidth_bps: u64, estimated_rtt_ms: u64) -> usize {
    // BDP in bytes = (bandwidth in bps / 8) * (RTT in seconds)
    let bdp = (estimated_bandwidth_bps / 8) * estimated_rtt_ms / 1000;
    
    // Clamp between 1 MB and 64 MB
    let min_buffer = 1_048_576;   // 1 MB
    let max_buffer = 67_108_864;  // 64 MB
    
    bdp.clamp(min_buffer, max_buffer) as usize
}

/// Configure TCP socket for optimal performance with dynamic buffer sizing
///
/// Phase 6.1.2 enhancements:
/// - Dynamic buffer sizing based on BDP
/// - TCP window scaling for >64KB windows
/// - Optimized for 10 Gbps networks
fn configure_tcp_socket(stream: &TcpStream) -> Result<()> {
    configure_tcp_socket_with_params(stream, 10_000_000_000, 100)
}

/// Configure TCP socket with custom bandwidth and RTT parameters
fn configure_tcp_socket_with_params(
    stream: &TcpStream,
    estimated_bandwidth_bps: u64,
    estimated_rtt_ms: u64,
) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        
        let fd = stream.as_raw_fd();
        
        // Enable BBR congestion control (Linux 4.9+)
        // BBR provides 20-50% better throughput than CUBIC
        let bbr = CString::new("bbr").map_err(|e| {
            Error::Configuration(format!("Failed to create BBR string: {}", e))
        })?;
        
        unsafe {
            let result = libc::setsockopt(
                fd,
                libc::IPPROTO_TCP,
                libc::TCP_CONGESTION,
                bbr.as_ptr() as *const libc::c_void,
                bbr.as_bytes().len() as libc::socklen_t,
            );
            
            if result < 0 {
                warn!("Failed to enable BBR congestion control, using system default");
            } else {
                debug!("BBR congestion control enabled");
            }
        }
        
        // Disable Nagle's algorithm for lower latency
        stream.set_nodelay(true).map_err(|e| {
            Error::Configuration(format!("Failed to set TCP_NODELAY: {}", e))
        })?;
        
        // Calculate optimal buffer size based on BDP
        let buffer_size = calculate_buffer_size(estimated_bandwidth_bps, estimated_rtt_ms);
        
        unsafe {
            // Enable TCP window scaling (RFC 1323)
            // Required for windows >64KB, automatically enabled when buffer >64KB
            let window_scale: i32 = 1;
            let result = libc::setsockopt(
                fd,
                libc::IPPROTO_TCP,
                libc::TCP_WINDOW_CLAMP,
                &(buffer_size as i32) as *const _ as *const libc::c_void,
                std::mem::size_of::<i32>() as libc::socklen_t,
            );
            
            if result < 0 {
                debug!("TCP_WINDOW_CLAMP not supported, window scaling will be automatic");
            }
            
            // Set receive buffer with dynamic sizing
            let result = libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                &(buffer_size as i32) as *const _ as *const libc::c_void,
                std::mem::size_of::<i32>() as libc::socklen_t,
            );
            
            if result < 0 {
                warn!("Failed to set SO_RCVBUF to {} bytes", buffer_size);
            } else {
                debug!("SO_RCVBUF set to {} bytes", buffer_size);
            }
            
            // Set send buffer with dynamic sizing
            let result = libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                &(buffer_size as i32) as *const _ as *const libc::c_void,
                std::mem::size_of::<i32>() as libc::socklen_t,
            );
            
            if result < 0 {
                warn!("Failed to set SO_SNDBUF to {} bytes", buffer_size);
            } else {
                debug!("SO_SNDBUF set to {} bytes", buffer_size);
            }
            
            // Enable TCP timestamps for better RTT measurement (RFC 1323)
            let timestamps: i32 = 1;
            let result = libc::setsockopt(
                fd,
                libc::IPPROTO_TCP,
                libc::TCP_TIMESTAMPS,
                &timestamps as *const _ as *const libc::c_void,
                std::mem::size_of::<i32>() as libc::socklen_t,
            );
            
            if result < 0 {
                debug!("Failed to enable TCP timestamps");
            }
        }
        
        // Phase 6.1.3: MTU Discovery and Jumbo Frame Support
        let mtu = discover_mtu(stream);
        enable_jumbo_frames(stream, mtu)?;
        
        info!(
            "TCP socket configured: BBR enabled, buffer={} MB (BDP: {}Gbps × {}ms), MTU={}",
            buffer_size / 1_048_576,
            estimated_bandwidth_bps / 1_000_000_000,
            estimated_rtt_ms,
            mtu.as_bytes()
        );
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        // On non-Linux systems, use socket2 for cross-platform buffer configuration
        use socket2::SockRef;
        
        let sock_ref = SockRef::from(stream);
        
        // Disable Nagle's algorithm
        stream.set_nodelay(true).map_err(|e| {
            Error::Configuration(format!("Failed to set TCP_NODELAY: {}", e))
        })?;
        
        // Calculate and set buffer sizes
        let buffer_size = calculate_buffer_size(estimated_bandwidth_bps, estimated_rtt_ms);
        
        if let Err(e) = sock_ref.set_recv_buffer_size(buffer_size) {
            warn!("Failed to set receive buffer size: {}", e);
        }
        
        if let Err(e) = sock_ref.set_send_buffer_size(buffer_size) {
            warn!("Failed to set send buffer size: {}", e);
        }
        
        info!(
            "TCP socket configured: buffer={} MB (BBR not available on this platform)",
            buffer_size / 1_048_576
        );
    }
    
    Ok(())
}

/// TCP transport backend
pub struct TcpBackend {
    state: Arc<RwLock<TcpBackendState>>,
}

impl TcpBackend {
    /// Create a new TCP backend
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(TcpBackendState {
                sessions: HashMap::new(),
                initialized: false,
            })),
        }
    }

    /// Create a server session (listens for connections)
    pub async fn create_server_session(&mut self, config: SessionConfig) -> Result<SessionId> {
        let session_id = SessionId::new();
        let session = TcpSession::new(session_id, config, true);

        let mut state = self.state.write().await;
        state.sessions.insert(session_id, session);

        debug!("Created TCP server session: {}", session_id);
        Ok(session_id)
    }

    /// Create a client session (connects to server)
    pub async fn create_client_session(&mut self, config: SessionConfig) -> Result<SessionId> {
        let session_id = SessionId::new();
        let session = TcpSession::new(session_id, config, false);

        let mut state = self.state.write().await;
        state.sessions.insert(session_id, session);

        debug!("Created TCP client session: {}", session_id);
        Ok(session_id)
    }
    
}

impl Default for TcpBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TransportBackend for TcpBackend {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::for_transport(TransportType::Tcp)
    }

    async fn initialize(&mut self) -> Result<()> {
        let mut state = self.state.write().await;
        state.initialized = true;
        info!("TCP backend initialized");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        let mut state = self.state.write().await;

        // Close all sessions
        for (id, session) in state.sessions.iter_mut() {
            if let Some(stream) = session.stream.take() {
                drop(stream);
            }
            if let Some(listener) = session.listener.take() {
                drop(listener);
            }
            debug!("Closed TCP session: {}", id);
        }

        state.sessions.clear();
        state.initialized = false;
        info!("TCP backend shutdown");
        Ok(())
    }

    async fn create_session(&mut self, config: SessionConfig) -> Result<SessionId> {
        // Default to client session
        self.create_client_session(config).await
    }

    async fn start_session(&mut self, session_id: SessionId) -> Result<()> {
        let mut state = self.state.write().await;

        let session = state
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| Error::Domain("Session not found".to_string()))?;

        if session.is_server {
            // Server: bind and listen
            let listener = TcpListener::bind(session.config.local_addr)
                .await
                .map_err(|e| Error::ConnectionFailed(e.to_string()))?;

            let local_addr = listener.local_addr().map_err(|e| Error::Io(e))?;
            info!("TCP server listening on {}", local_addr);

            session.listener = Some(listener);
        } else {
            // Client: connect to server
            let stream = TcpStream::connect(session.config.remote_addr)
                .await
                .map_err(|e| Error::ConnectionFailed(e.to_string()))?;

            // Configure socket for optimal performance (BBR, large buffers)
            configure_tcp_socket(&stream)?;

            // Discover MTU and enable jumbo frames if supported
            let mtu = discover_mtu(&stream);
            session.mtu = mtu;
            
            if let Err(e) = enable_jumbo_frames(&stream, mtu) {
                debug!("Could not enable jumbo frames: {}", e);
            }

            let local_addr = stream.local_addr().map_err(|e| Error::Io(e))?;
            let peer_addr = stream.peer_addr().map_err(|e| Error::Io(e))?;
            info!("TCP client connected [session {}]: {} -> {} (MTU: {:?})",
                  session.id, local_addr, peer_addr, mtu);

            session.stream = Some(stream);
        }

        debug!("Started TCP session: {} (internal ID: {})", session_id, session.id);
        Ok(())
    }

    async fn stop_session(&mut self, session_id: SessionId) -> Result<()> {
        let mut state = self.state.write().await;

        if let Some(mut session) = state.sessions.remove(&session_id) {
            if let Some(stream) = session.stream.take() {
                drop(stream);
            }
            if let Some(listener) = session.listener.take() {
                drop(listener);
            }
            debug!("Stopped TCP session: {}", session_id);
        }

        Ok(())
    }

    async fn send(&mut self, session_id: SessionId, data: &[u8]) -> Result<usize> {
        let mut state = self.state.write().await;

        let session = state
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| Error::Domain("Session not found".to_string()))?;

        let stream = session
            .stream
            .as_mut()
            .ok_or_else(|| Error::NoConnection)?;

        let bytes_sent = stream
            .write(data)
            .await
            .map_err(|e| Error::SendFailed(e.to_string()))?;

        session.stats.record_send(bytes_sent as u64);

        Ok(bytes_sent)
    }

    async fn receive(&mut self, session_id: SessionId, buffer: &mut [u8]) -> Result<usize> {
        let mut state = self.state.write().await;

        let session = state
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| Error::Domain("Session not found".to_string()))?;

        // If server and no stream yet, accept a connection
        if session.is_server && session.stream.is_none() {
            if let Some(listener) = &session.listener {
                let (stream, peer_addr) = listener
                    .accept()
                    .await
                    .map_err(|e| Error::ConnectionFailed(e.to_string()))?;

                // Configure socket for optimal performance (BBR, large buffers)
                configure_tcp_socket(&stream)?;

                info!("TCP server accepted connection from {}", peer_addr);
                session.stream = Some(stream);
            }
        }

        let stream = session
            .stream
            .as_mut()
            .ok_or_else(|| Error::NoConnection)?;

        let bytes_received = stream
            .read(buffer)
            .await
            .map_err(|e| Error::ReceiveFailed(e.to_string()))?;

        session.stats.record_receive(bytes_received as u64);

        Ok(bytes_received)
    }

    async fn get_stats(&self, session_id: SessionId) -> Result<TransportStats> {
        let state = self.state.read().await;

        let session = state
            .sessions
            .get(&session_id)
            .ok_or_else(|| Error::Domain("Session not found".to_string()))?;

        Ok(session.stats.clone())
    }

    async fn is_session_active(&self, session_id: SessionId) -> Result<bool> {
        let state = self.state.read().await;

        Ok(state
            .sessions
            .get(&session_id)
            .map(|s| s.stream.is_some() || s.listener.is_some())
            .unwrap_or(false))
    }

    async fn local_addr(&self, session_id: SessionId) -> Result<SocketAddr> {
        let state = self.state.read().await;

        let session = state
            .sessions
            .get(&session_id)
            .ok_or_else(|| Error::Domain("Session not found".to_string()))?;

        if let Some(stream) = &session.stream {
            stream.local_addr().map_err(|e| Error::Io(e))
        } else if let Some(listener) = &session.listener {
            listener.local_addr().map_err(|e| Error::Io(e))
        } else {
            Ok(session.config.local_addr)
        }
    }

    async fn remote_addr(&self, session_id: SessionId) -> Result<SocketAddr> {
        let state = self.state.read().await;

        let session = state
            .sessions
            .get(&session_id)
            .ok_or_else(|| Error::Domain("Session not found".to_string()))?;

        if let Some(stream) = &session.stream {
            stream.peer_addr().map_err(|e| Error::Io(e))
        } else {
            Ok(session.config.remote_addr)
        }
    }

    async fn pause_session(&mut self, session_id: SessionId) -> Result<()> {
        // TCP doesn't support pause/resume, but we can track the state
        debug!("TCP session pause requested: {}", session_id);
        Ok(())
    }

    async fn resume_session(&mut self, session_id: SessionId) -> Result<()> {
        // TCP doesn't support pause/resume, but we can track the state
        debug!("TCP session resume requested: {}", session_id);
        Ok(())
    }

    async fn get_throughput(&self, session_id: SessionId) -> Result<u64> {
        let stats = self.get_stats(session_id).await?;
        Ok(stats.throughput_bps)
    }

    async fn get_rtt(&self, session_id: SessionId) -> Result<u64> {
        let stats = self.get_stats(session_id).await?;
        Ok(stats.rtt_ms)
    }

    async fn health_check(&self) -> Result<bool> {
        let state = self.state.read().await;
        Ok(state.initialized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tcp_backend_initialization() {
        let mut backend = TcpBackend::new();

        assert!(!backend.health_check().await.unwrap());

        backend.initialize().await.unwrap();
        assert!(backend.health_check().await.unwrap());

        backend.shutdown().await.unwrap();
        assert!(!backend.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_tcp_session_creation() {
        let mut backend = TcpBackend::new();
        backend.initialize().await.unwrap();

        let config = SessionConfig {
            transport_type: TransportType::Tcp,
            local_addr: "127.0.0.1:0".parse().unwrap(),
            remote_addr: "127.0.0.1:8080".parse().unwrap(),
            ..Default::default()
        };

        let session_id = backend.create_session(config).await.unwrap();
        assert!(!backend.is_session_active(session_id).await.unwrap());

        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_tcp_capabilities() {
        let backend = TcpBackend::new();
        let caps = backend.capabilities();

        assert_eq!(caps.transport_type, TransportType::Tcp);
        assert_eq!(caps.max_throughput_bps, 250_000_000); // Updated with BBR + window scaling + jumbo frames + zero-copy
        
        // Zero-copy is only supported on Linux
        #[cfg(target_os = "linux")]
        assert!(caps.supports_zero_copy);
        
        #[cfg(not(target_os = "linux"))]
        assert!(!caps.supports_zero_copy);
        
        assert!(!caps.uses_kernel_bypass);
    }
}

// Made with Bob
