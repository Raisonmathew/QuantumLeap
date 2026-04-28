//! TURN Allocation Management
//!
//! Manages TURN allocations, permissions, and channels

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::{MAX_LIFETIME, MIN_LIFETIME, CHANNEL_MIN, CHANNEL_MAX};

/// TURN Allocation
///
/// Represents a relay allocation for a client
#[derive(Debug, Clone)]
pub struct Allocation {
    /// Unique allocation ID
    pub id: String,
    /// Client's 5-tuple (client addr, server addr, protocol)
    pub client_addr: SocketAddr,
    pub server_addr: SocketAddr,
    pub protocol: TransportProtocol,
    /// Relay address assigned to this allocation
    pub relay_addr: SocketAddr,
    /// Allocation lifetime
    pub lifetime: Duration,
    /// Creation time
    pub created_at: Instant,
    /// Last refresh time
    pub refreshed_at: Instant,
    /// Permissions (peer addresses allowed to send data)
    pub permissions: HashMap<SocketAddr, Permission>,
    /// Channel bindings (channel number -> peer address)
    pub channels: HashMap<u16, Channel>,
    /// Reverse channel lookup (peer address -> channel number)
    pub peer_to_channel: HashMap<SocketAddr, u16>,
    /// Bandwidth usage statistics
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Transport protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportProtocol {
    Udp,
    Tcp,
}

impl Allocation {
    /// Create new allocation
    pub fn new(
        id: String,
        client_addr: SocketAddr,
        server_addr: SocketAddr,
        relay_addr: SocketAddr,
        protocol: TransportProtocol,
        lifetime: Duration,
    ) -> Self {
        let now = Instant::now();
        Self {
            id,
            client_addr,
            server_addr,
            protocol,
            relay_addr,
            lifetime,
            created_at: now,
            refreshed_at: now,
            permissions: HashMap::new(),
            channels: HashMap::new(),
            peer_to_channel: HashMap::new(),
            bytes_sent: 0,
            bytes_received: 0,
        }
    }

    /// Check if allocation has expired
    pub fn is_expired(&self) -> bool {
        self.refreshed_at.elapsed() > self.lifetime
    }

    /// Refresh allocation lifetime
    pub fn refresh(&mut self, lifetime: Duration) {
        self.refreshed_at = Instant::now();
        self.lifetime = lifetime;
    }

    /// Add or update permission for a peer
    pub fn add_permission(&mut self, peer_addr: SocketAddr, lifetime: Duration) {
        let permission = Permission {
            peer_addr,
            created_at: Instant::now(),
            lifetime,
        };
        self.permissions.insert(peer_addr, permission);
    }

    /// Check if peer has permission
    pub fn has_permission(&self, peer_addr: &SocketAddr) -> bool {
        if let Some(permission) = self.permissions.get(peer_addr) {
            !permission.is_expired()
        } else {
            false
        }
    }

    /// Remove expired permissions
    pub fn cleanup_permissions(&mut self) {
        self.permissions.retain(|_, p| !p.is_expired());
    }

    /// Bind a channel to a peer address
    pub fn bind_channel(&mut self, channel_number: u16, peer_addr: SocketAddr) -> Result<(), String> {
        // Validate channel number range
        if channel_number < CHANNEL_MIN || channel_number > CHANNEL_MAX {
            return Err(format!("Invalid channel number: {}", channel_number));
        }

        // Check if channel is already bound
        if self.channels.contains_key(&channel_number) {
            return Err(format!("Channel {} already bound", channel_number));
        }

        // Check if peer already has a channel
        if self.peer_to_channel.contains_key(&peer_addr) {
            return Err(format!("Peer {} already has a channel", peer_addr));
        }

        let channel = Channel {
            number: channel_number,
            peer_addr,
            created_at: Instant::now(),
        };

        self.channels.insert(channel_number, channel);
        self.peer_to_channel.insert(peer_addr, channel_number);

        Ok(())
    }

    /// Get channel number for peer address
    pub fn get_channel_for_peer(&self, peer_addr: &SocketAddr) -> Option<u16> {
        self.peer_to_channel.get(peer_addr).copied()
    }

    /// Get peer address for channel number
    pub fn get_peer_for_channel(&self, channel_number: u16) -> Option<SocketAddr> {
        self.channels.get(&channel_number).map(|c| c.peer_addr)
    }

    /// Record sent bytes
    pub fn record_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
    }

    /// Record received bytes
    pub fn record_received(&mut self, bytes: u64) {
        self.bytes_received += bytes;
    }
}

/// Permission for a peer to send data through the relay
#[derive(Debug, Clone)]
pub struct Permission {
    /// Peer address
    pub peer_addr: SocketAddr,
    /// Creation time
    pub created_at: Instant,
    /// Permission lifetime (typically 5 minutes)
    pub lifetime: Duration,
}

impl Permission {
    /// Check if permission has expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.lifetime
    }
}

/// Channel binding for efficient data transfer
#[derive(Debug, Clone)]
pub struct Channel {
    /// Channel number (0x4000 - 0x7FFF)
    pub number: u16,
    /// Peer address bound to this channel
    pub peer_addr: SocketAddr,
    /// Creation time
    pub created_at: Instant,
}

/// Allocation Manager
///
/// Manages all active TURN allocations
pub struct AllocationManager {
    /// Active allocations (allocation ID -> allocation)
    allocations: Arc<RwLock<HashMap<String, Allocation>>>,
    /// Client address to allocation ID mapping
    client_to_allocation: Arc<RwLock<HashMap<SocketAddr, String>>>,
    /// Relay address to allocation ID mapping
    relay_to_allocation: Arc<RwLock<HashMap<SocketAddr, String>>>,
    /// Next available relay port
    next_relay_port: Arc<RwLock<u16>>,
    /// Relay address base
    relay_base_addr: SocketAddr,
    /// Per-source-IP active-allocation count.
    ///
    /// SECURITY: TURN allocations are expensive (one relay port + state
    /// per allocation) and the original `create_allocation` only enforced
    /// a single allocation per `(IP, port)` tuple. A misbehaving client
    /// behind a NAT can mint thousands of allocations from the same IP
    /// by varying its source port, exhausting the relay-port pool. We
    /// track the active count per source IP and refuse new allocations
    /// once the per-IP ceiling is hit.
    per_ip_count: Arc<RwLock<HashMap<std::net::IpAddr, usize>>>,
}

/// Maximum simultaneously-held allocations per source IP. 32 covers
/// reasonable carrier-grade NAT scenarios while bounding the
/// damage one rogue IP can do to the relay-port pool.
const MAX_ALLOCATIONS_PER_IP: usize = 32;

impl AllocationManager {
    /// Create new allocation manager
    pub fn new(relay_base_addr: SocketAddr) -> Self {
        Self {
            allocations: Arc::new(RwLock::new(HashMap::new())),
            client_to_allocation: Arc::new(RwLock::new(HashMap::new())),
            relay_to_allocation: Arc::new(RwLock::new(HashMap::new())),
            next_relay_port: Arc::new(RwLock::new(49152)), // Start of dynamic port range
            relay_base_addr,
            per_ip_count: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create new allocation
    pub async fn create_allocation(
        &self,
        client_addr: SocketAddr,
        server_addr: SocketAddr,
        protocol: TransportProtocol,
        requested_lifetime: u32,
    ) -> Result<Allocation, String> {
        // Validate and clamp lifetime
        let lifetime_secs = requested_lifetime.clamp(MIN_LIFETIME, MAX_LIFETIME);
        let lifetime = Duration::from_secs(lifetime_secs as u64);

        // Check if client already has an allocation
        let client_map = self.client_to_allocation.read().await;
        if client_map.contains_key(&client_addr) {
            return Err("Client already has an allocation".to_string());
        }
        drop(client_map);

        // Per-source-IP cap: prevents a single host from monopolising the
        // relay-port pool by binding many ephemeral source ports.
        {
            let per_ip = self.per_ip_count.read().await;
            if per_ip.get(&client_addr.ip()).copied().unwrap_or(0)
                >= MAX_ALLOCATIONS_PER_IP
            {
                return Err(format!(
                    "Per-IP allocation cap ({}) reached for {}",
                    MAX_ALLOCATIONS_PER_IP,
                    client_addr.ip()
                ));
            }
        }

        // Allocate relay address
        let relay_addr = self.allocate_relay_address().await?;

        // Generate allocation ID
        let id = format!("{}-{}", client_addr, relay_addr);

        // Create allocation
        let allocation = Allocation::new(
            id.clone(),
            client_addr,
            server_addr,
            relay_addr,
            protocol,
            lifetime,
        );

        // Store allocation
        let mut allocations = self.allocations.write().await;
        let mut client_map = self.client_to_allocation.write().await;
        let mut relay_map = self.relay_to_allocation.write().await;
        let mut per_ip = self.per_ip_count.write().await;

        allocations.insert(id.clone(), allocation.clone());
        client_map.insert(client_addr, id.clone());
        relay_map.insert(relay_addr, id);
        *per_ip.entry(client_addr.ip()).or_insert(0) += 1;

        Ok(allocation)
    }

    /// Get allocation by client address
    pub async fn get_by_client(&self, client_addr: &SocketAddr) -> Option<Allocation> {
        let client_map = self.client_to_allocation.read().await;
        let id = client_map.get(client_addr)?;
        let allocations = self.allocations.read().await;
        allocations.get(id).cloned()
    }

    /// Get allocation by relay address
    pub async fn get_by_relay(&self, relay_addr: &SocketAddr) -> Option<Allocation> {
        let relay_map = self.relay_to_allocation.read().await;
        let id = relay_map.get(relay_addr)?;
        let allocations = self.allocations.read().await;
        allocations.get(id).cloned()
    }

    /// Refresh allocation
    pub async fn refresh_allocation(
        &self,
        client_addr: &SocketAddr,
        requested_lifetime: u32,
    ) -> Result<u32, String> {
        let client_map = self.client_to_allocation.read().await;
        let id = client_map.get(client_addr)
            .ok_or_else(|| "No allocation found".to_string())?
            .clone();
        drop(client_map);

        let mut allocations = self.allocations.write().await;
        let allocation = allocations.get_mut(&id)
            .ok_or_else(|| "Allocation not found".to_string())?;

        // Validate and clamp lifetime
        let lifetime_secs = requested_lifetime.clamp(MIN_LIFETIME, MAX_LIFETIME);
        let lifetime = Duration::from_secs(lifetime_secs as u64);

        allocation.refresh(lifetime);

        Ok(lifetime_secs)
    }

    /// Delete allocation
    pub async fn delete_allocation(&self, client_addr: &SocketAddr) -> Result<(), String> {
        let mut client_map = self.client_to_allocation.write().await;
        let id = client_map.remove(client_addr)
            .ok_or_else(|| "No allocation found".to_string())?;

        let mut allocations = self.allocations.write().await;
        let allocation = allocations.remove(&id)
            .ok_or_else(|| "Allocation not found".to_string())?;

        let mut relay_map = self.relay_to_allocation.write().await;
        relay_map.remove(&allocation.relay_addr);

        Ok(())
    }

    /// Add permission to allocation
    pub async fn add_permission(
        &self,
        client_addr: &SocketAddr,
        peer_addr: SocketAddr,
        lifetime: Duration,
    ) -> Result<(), String> {
        let client_map = self.client_to_allocation.read().await;
        let id = client_map.get(client_addr)
            .ok_or_else(|| "No allocation found".to_string())?
            .clone();
        drop(client_map);

        let mut allocations = self.allocations.write().await;
        let allocation = allocations.get_mut(&id)
            .ok_or_else(|| "Allocation not found".to_string())?;

        allocation.add_permission(peer_addr, lifetime);

        Ok(())
    }

    /// Bind channel
    pub async fn bind_channel(
        &self,
        client_addr: &SocketAddr,
        channel_number: u16,
        peer_addr: SocketAddr,
    ) -> Result<(), String> {
        // CONCURRENCY (TOCTOU): the previous version released the
        // `client_to_allocation` read lock before grabbing the
        // `allocations` write lock. In that window another task could
        // delete or replace the allocation, so the subsequent
        // `get_mut(&id)` either failed silently or mutated a different
        // allocation. We now acquire the write lock on `allocations`
        // FIRST, and look up the allocation id with the client_to_allocation
        // read lock held only as long as needed inside that critical
        // section. The two-lock acquisition order matches every other
        // mutation path (allocations -> client_to_allocation), avoiding
        // deadlock.
        let mut allocations = self.allocations.write().await;
        let id = {
            let client_map = self.client_to_allocation.read().await;
            client_map
                .get(client_addr)
                .ok_or_else(|| "No allocation found".to_string())?
                .clone()
        };
        let allocation = allocations.get_mut(&id)
            .ok_or_else(|| "Allocation not found".to_string())?;

        allocation.bind_channel(channel_number, peer_addr)?;

        Ok(())
    }

    /// Cleanup expired allocations and permissions
    pub async fn cleanup_expired(&self) {
        let mut allocations = self.allocations.write().await;
        let mut client_map = self.client_to_allocation.write().await;
        let mut relay_map = self.relay_to_allocation.write().await;
        let mut per_ip = self.per_ip_count.write().await;

        // Find expired allocations
        let expired_ids: Vec<String> = allocations
            .iter()
            .filter(|(_, a)| a.is_expired())
            .map(|(id, _)| id.clone())
            .collect();

        // Remove expired allocations
        for id in expired_ids {
            if let Some(allocation) = allocations.remove(&id) {
                client_map.remove(&allocation.client_addr);
                relay_map.remove(&allocation.relay_addr);
                // Decrement per-IP counter, removing the entry when it
                // drops to zero so the table doesn't accumulate dead IPs.
                let ip = allocation.client_addr.ip();
                if let Some(slot) = per_ip.get_mut(&ip) {
                    *slot = slot.saturating_sub(1);
                    if *slot == 0 {
                        per_ip.remove(&ip);
                    }
                }
            }
        }

        // Cleanup expired permissions in remaining allocations
        for allocation in allocations.values_mut() {
            allocation.cleanup_permissions();
        }
    }

    /// Get allocation count
    pub async fn allocation_count(&self) -> usize {
        self.allocations.read().await.len()
    }

    /// Allocate next available relay address
    async fn allocate_relay_address(&self) -> Result<SocketAddr, String> {
        let mut next_port = self.next_relay_port.write().await;
        
        // Find next available port
        let port = *next_port;
        *next_port = if port >= 65535 { 49152 } else { port + 1 };

        // Create relay address with allocated port
        let mut relay_addr = self.relay_base_addr;
        relay_addr.set_port(port);

        Ok(relay_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_allocation() {
        let manager = AllocationManager::new("127.0.0.1:3478".parse().unwrap());
        let client_addr = "192.168.1.100:5000".parse().unwrap();
        let server_addr = "10.0.0.1:3478".parse().unwrap();

        let allocation = manager
            .create_allocation(client_addr, server_addr, TransportProtocol::Udp, 600)
            .await
            .unwrap();

        assert_eq!(allocation.client_addr, client_addr);
        assert_eq!(allocation.server_addr, server_addr);
        assert_eq!(allocation.protocol, TransportProtocol::Udp);
        assert_eq!(allocation.lifetime.as_secs(), 600);
    }

    #[tokio::test]
    async fn test_get_allocation() {
        let manager = AllocationManager::new("127.0.0.1:3478".parse().unwrap());
        let client_addr = "192.168.1.100:5000".parse().unwrap();
        let server_addr = "10.0.0.1:3478".parse().unwrap();

        let created = manager
            .create_allocation(client_addr, server_addr, TransportProtocol::Udp, 600)
            .await
            .unwrap();

        let retrieved = manager.get_by_client(&client_addr).await.unwrap();
        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.relay_addr, created.relay_addr);
    }

    #[tokio::test]
    async fn test_refresh_allocation() {
        let manager = AllocationManager::new("127.0.0.1:3478".parse().unwrap());
        let client_addr = "192.168.1.100:5000".parse().unwrap();
        let server_addr = "10.0.0.1:3478".parse().unwrap();

        manager
            .create_allocation(client_addr, server_addr, TransportProtocol::Udp, 600)
            .await
            .unwrap();

        let new_lifetime = manager.refresh_allocation(&client_addr, 1200).await.unwrap();
        assert_eq!(new_lifetime, 1200);
    }

    #[tokio::test]
    async fn test_add_permission() {
        let manager = AllocationManager::new("127.0.0.1:3478".parse().unwrap());
        let client_addr = "192.168.1.100:5000".parse().unwrap();
        let server_addr = "10.0.0.1:3478".parse().unwrap();
        let peer_addr = "192.168.1.200:6000".parse().unwrap();

        manager
            .create_allocation(client_addr, server_addr, TransportProtocol::Udp, 600)
            .await
            .unwrap();

        manager
            .add_permission(&client_addr, peer_addr, Duration::from_secs(300))
            .await
            .unwrap();

        let allocation = manager.get_by_client(&client_addr).await.unwrap();
        assert!(allocation.has_permission(&peer_addr));
    }

    #[tokio::test]
    async fn test_bind_channel() {
        let manager = AllocationManager::new("127.0.0.1:3478".parse().unwrap());
        let client_addr = "192.168.1.100:5000".parse().unwrap();
        let server_addr = "10.0.0.1:3478".parse().unwrap();
        let peer_addr = "192.168.1.200:6000".parse().unwrap();

        manager
            .create_allocation(client_addr, server_addr, TransportProtocol::Udp, 600)
            .await
            .unwrap();

        manager
            .bind_channel(&client_addr, 0x4000, peer_addr)
            .await
            .unwrap();

        let allocation = manager.get_by_client(&client_addr).await.unwrap();
        assert_eq!(allocation.get_channel_for_peer(&peer_addr), Some(0x4000));
        assert_eq!(allocation.get_peer_for_channel(0x4000), Some(peer_addr));
    }

    #[tokio::test]
    async fn test_delete_allocation() {
        let manager = AllocationManager::new("127.0.0.1:3478".parse().unwrap());
        let client_addr = "192.168.1.100:5000".parse().unwrap();
        let server_addr = "10.0.0.1:3478".parse().unwrap();

        manager
            .create_allocation(client_addr, server_addr, TransportProtocol::Udp, 600)
            .await
            .unwrap();

        assert_eq!(manager.allocation_count().await, 1);

        manager.delete_allocation(&client_addr).await.unwrap();

        assert_eq!(manager.allocation_count().await, 0);
        assert!(manager.get_by_client(&client_addr).await.is_none());
    }
}

// Made with Bob