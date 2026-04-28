//! Token-bucket rate limiter for protecting authentication and licensing
//! endpoints against credential-stuffing and key-brute-force attacks.
//!
//! # Algorithm
//!
//! Standard token bucket: each tracked key holds a `(tokens, last_refill)`
//! pair. Tokens refill at `refill_per_sec` up to `capacity`. A request
//! consumes one token; if fewer than one is available, the request is
//! rejected with `RateLimited`. This naturally allows short bursts up to
//! `capacity` while bounding long-term throughput at `refill_per_sec`.
//!
//! # Memory bound
//!
//! `RateLimiter` caps the live-key count at `max_keys`. When that ceiling
//! is hit, the LRU entry is evicted. This stops attackers from inflating
//! memory by submitting endless distinct usernames / license keys.
//!
//! # Threading
//!
//! Internal state is protected by a single `Mutex`. For the request rates
//! these endpoints see (logins / activations) the contention is
//! negligible compared to the cost of an Argon2 verify or an SQLite
//! round-trip that follows.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Configuration for a `RateLimiter`.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    /// Maximum tokens (= maximum burst size).
    pub capacity: u32,
    /// Tokens refilled per second.
    pub refill_per_sec: f64,
    /// Maximum number of distinct keys to track simultaneously.
    pub max_keys: usize,
}

impl RateLimitConfig {
    /// Sensible default for interactive login: 5 attempts burst, 1 per
    /// minute sustained, 100 000 distinct usernames tracked.
    pub const fn login_default() -> Self {
        Self {
            capacity: 5,
            refill_per_sec: 1.0 / 60.0,
            max_keys: 100_000,
        }
    }

    /// Sensible default for license activation: 10 attempts burst, 1 per
    /// 10 seconds sustained, 100 000 distinct keys tracked.
    pub const fn license_activation_default() -> Self {
        Self {
            capacity: 10,
            refill_per_sec: 0.1,
            max_keys: 100_000,
        }
    }
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last_refill: Instant,
    last_used: Instant,
}

/// Per-key token-bucket rate limiter. Cloning is intentionally not
/// implemented \u2014 wrap in `Arc` to share.
pub struct RateLimiter {
    config: RateLimitConfig,
    state: Mutex<HashMap<String, Bucket>>,
}

impl RateLimiter {
    /// Build a new limiter from the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Attempt to consume one token for `key`.
    ///
    /// Returns `Ok(())` on success and `Err(RateLimited { retry_after })`
    /// when no token is available, where `retry_after` is the wall-clock
    /// duration the caller should wait before its next attempt.
    pub fn check(&self, key: &str) -> Result<(), RateLimited> {
        self.check_at(key, Instant::now())
    }

    fn check_at(&self, key: &str, now: Instant) -> Result<(), RateLimited> {
        // RELIABILITY: a panic in any prior holder of this lock would
        // poison it. Previously we panicked again with `.expect(...)`,
        // turning a single bad request into a permanent service outage.
        // The bucket data is plain numeric state with no invariants that
        // a half-completed mutation could violate, so it is safe to
        // recover the inner guard via `into_inner()` and continue.
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());

        // Bound live key count: when full, evict the bucket least recently
        // touched. O(n) but `max_keys` is large and this only fires when
        // the table is genuinely saturated.
        if !state.contains_key(key) && state.len() >= self.config.max_keys {
            if let Some(victim) = state
                .iter()
                .min_by_key(|(_, b)| b.last_used)
                .map(|(k, _)| k.clone())
            {
                state.remove(&victim);
            }
        }

        let bucket = state.entry(key.to_string()).or_insert(Bucket {
            tokens: self.config.capacity as f64,
            last_refill: now,
            last_used: now,
        });

        // Refill since last touch, clamped to capacity.
        let elapsed = now.saturating_duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens =
            (bucket.tokens + elapsed * self.config.refill_per_sec).min(self.config.capacity as f64);
        bucket.last_refill = now;
        bucket.last_used = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            // Tokens needed = 1 - bucket.tokens; time to refill that many
            // = (1 - bucket.tokens) / refill_per_sec.
            let deficit = 1.0 - bucket.tokens;
            let secs = deficit / self.config.refill_per_sec.max(f64::EPSILON);
            Err(RateLimited {
                retry_after: Duration::from_secs_f64(secs),
            })
        }
    }

    /// Number of distinct keys currently held (testing/observability).
    pub fn tracked_keys(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }
}

/// Returned when the limiter rejects an attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimited {
    /// Suggested wait before the next attempt is likely to succeed.
    pub retry_after: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(capacity: u32, refill: f64) -> RateLimitConfig {
        RateLimitConfig {
            capacity,
            refill_per_sec: refill,
            max_keys: 1024,
        }
    }

    #[test]
    fn allows_burst_up_to_capacity() {
        let rl = RateLimiter::new(cfg(3, 1.0));
        for _ in 0..3 {
            assert!(rl.check("k").is_ok());
        }
        assert!(rl.check("k").is_err());
    }

    #[test]
    fn refills_over_time() {
        let rl = RateLimiter::new(cfg(2, 100.0)); // 100 tokens/sec
        let t0 = Instant::now();
        assert!(rl.check_at("k", t0).is_ok());
        assert!(rl.check_at("k", t0).is_ok());
        assert!(rl.check_at("k", t0).is_err());
        // 50 ms later: ~5 tokens accumulated, capped to 2.
        assert!(rl.check_at("k", t0 + Duration::from_millis(50)).is_ok());
    }

    #[test]
    fn distinct_keys_have_independent_buckets() {
        let rl = RateLimiter::new(cfg(1, 0.0));
        assert!(rl.check("alice").is_ok());
        assert!(rl.check("alice").is_err());
        assert!(rl.check("bob").is_ok()); // unaffected
    }

    #[test]
    fn evicts_lru_when_max_keys_reached() {
        let rl = RateLimiter::new(RateLimitConfig {
            capacity: 1,
            refill_per_sec: 0.0,
            max_keys: 2,
        });
        let t0 = Instant::now();
        rl.check_at("a", t0).unwrap();
        rl.check_at("b", t0 + Duration::from_millis(1)).unwrap();
        // Adding "c" should evict the oldest ("a").
        rl.check_at("c", t0 + Duration::from_millis(2)).unwrap();
        assert_eq!(rl.tracked_keys(), 2);
    }

    #[test]
    fn rate_limited_carries_retry_after() {
        let rl = RateLimiter::new(cfg(1, 1.0));
        rl.check("k").unwrap();
        let err = rl.check("k").unwrap_err();
        assert!(err.retry_after > Duration::ZERO);
        assert!(err.retry_after <= Duration::from_secs(2));
    }
}

// Made with Bob
