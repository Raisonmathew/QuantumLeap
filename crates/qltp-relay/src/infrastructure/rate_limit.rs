//! Rate Limiting Module
//!
//! Provides rate limiting for STUN/TURN servers to prevent abuse

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: usize,
    /// Time window duration
    pub window_duration: Duration,
    /// Cleanup interval for expired entries
    pub cleanup_interval: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window_duration: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        }
    }
}

/// Request tracking entry
#[derive(Debug, Clone)]
struct RequestEntry {
    count: usize,
    window_start: Instant,
}

/// Rate limiter
pub struct RateLimiter {
    config: RateLimitConfig,
    /// IP address -> request tracking
    requests: Arc<RwLock<HashMap<IpAddr, RequestEntry>>>,
}

impl RateLimiter {
    /// Create new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if request is allowed
    pub async fn check_rate_limit(&self, ip: IpAddr) -> bool {
        let mut requests = self.requests.write().await;
        let now = Instant::now();

        let entry = requests.entry(ip).or_insert(RequestEntry {
            count: 0,
            window_start: now,
        });

        // Check if window has expired
        if now.duration_since(entry.window_start) > self.config.window_duration {
            // Reset window
            entry.count = 1;
            entry.window_start = now;
            true
        } else if entry.count < self.config.max_requests {
            // Within limit
            entry.count += 1;
            true
        } else {
            // Rate limit exceeded
            false
        }
    }

    /// Get current request count for IP
    pub async fn get_request_count(&self, ip: &IpAddr) -> usize {
        let requests = self.requests.read().await;
        requests.get(ip).map(|e| e.count).unwrap_or(0)
    }

    /// Cleanup expired entries
    pub async fn cleanup(&self) {
        let mut requests = self.requests.write().await;
        let now = Instant::now();

        requests.retain(|_, entry| {
            now.duration_since(entry.window_start) <= self.config.window_duration
        });
    }

    /// Get total tracked IPs
    pub async fn tracked_ips(&self) -> usize {
        let requests = self.requests.read().await;
        requests.len()
    }

    /// Reset rate limit for IP
    pub async fn reset(&self, ip: &IpAddr) {
        let mut requests = self.requests.write().await;
        requests.remove(ip);
    }

    /// Clear all rate limits
    pub async fn clear_all(&self) {
        let mut requests = self.requests.write().await;
        requests.clear();
    }
}

/// Start background cleanup task
pub fn start_cleanup_task(rate_limiter: Arc<RateLimiter>) -> tokio::task::JoinHandle<()> {
    let cleanup_interval = rate_limiter.config.cleanup_interval;
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(cleanup_interval);
        loop {
            interval.tick().await;
            rate_limiter.cleanup().await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_allow() {
        let config = RateLimitConfig {
            max_requests: 5,
            window_duration: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        };
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.100".parse().unwrap();

        // First 5 requests should be allowed
        for _ in 0..5 {
            assert!(limiter.check_rate_limit(ip).await);
        }

        // 6th request should be denied
        assert!(!limiter.check_rate_limit(ip).await);
    }

    #[tokio::test]
    async fn test_rate_limit_window_reset() {
        let config = RateLimitConfig {
            max_requests: 2,
            window_duration: Duration::from_millis(100),
            cleanup_interval: Duration::from_secs(300),
        };
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.100".parse().unwrap();

        // Use up the limit
        assert!(limiter.check_rate_limit(ip).await);
        assert!(limiter.check_rate_limit(ip).await);
        assert!(!limiter.check_rate_limit(ip).await);

        // Wait for window to expire
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be allowed again
        assert!(limiter.check_rate_limit(ip).await);
    }

    #[tokio::test]
    async fn test_get_request_count() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.100".parse().unwrap();

        assert_eq!(limiter.get_request_count(&ip).await, 0);

        limiter.check_rate_limit(ip).await;
        assert_eq!(limiter.get_request_count(&ip).await, 1);

        limiter.check_rate_limit(ip).await;
        assert_eq!(limiter.get_request_count(&ip).await, 2);
    }

    #[tokio::test]
    async fn test_reset() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.100".parse().unwrap();

        limiter.check_rate_limit(ip).await;
        assert_eq!(limiter.get_request_count(&ip).await, 1);

        limiter.reset(&ip).await;
        assert_eq!(limiter.get_request_count(&ip).await, 0);
    }

    #[tokio::test]
    async fn test_multiple_ips() {
        let config = RateLimitConfig {
            max_requests: 2,
            window_duration: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        };
        let limiter = RateLimiter::new(config);
        let ip1: IpAddr = "192.168.1.100".parse().unwrap();
        let ip2: IpAddr = "192.168.1.101".parse().unwrap();

        // Each IP has its own limit
        assert!(limiter.check_rate_limit(ip1).await);
        assert!(limiter.check_rate_limit(ip1).await);
        assert!(!limiter.check_rate_limit(ip1).await);

        assert!(limiter.check_rate_limit(ip2).await);
        assert!(limiter.check_rate_limit(ip2).await);
        assert!(!limiter.check_rate_limit(ip2).await);

        assert_eq!(limiter.tracked_ips().await, 2);
    }

    #[tokio::test]
    async fn test_cleanup() {
        let config = RateLimitConfig {
            max_requests: 10,
            window_duration: Duration::from_millis(50),
            cleanup_interval: Duration::from_secs(300),
        };
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.100".parse().unwrap();

        limiter.check_rate_limit(ip).await;
        assert_eq!(limiter.tracked_ips().await, 1);

        // Wait for window to expire
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Cleanup should remove expired entry
        limiter.cleanup().await;
        assert_eq!(limiter.tracked_ips().await, 0);
    }
}

// Made with Bob