//! Fallback Manager - Automatic backend fallback and retry logic
//!
//! Handles backend failures gracefully by automatically falling back to
//! alternative transports with exponential backoff retry logic.

use crate::application::{BackendSelector, SelectionCriteria, SelectionResult};
use crate::domain::TransportType;
use crate::error::{Error, Result};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts per backend
    pub max_retries: usize,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier (exponential)
    pub backoff_multiplier: f64,
    /// Enable jitter to prevent thundering herd
    pub enable_jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            enable_jitter: true,
        }
    }
}

/// Fallback attempt result
#[derive(Debug, Clone)]
pub struct FallbackAttempt {
    /// Transport type attempted
    pub transport_type: TransportType,
    /// Attempt number (1-based)
    pub attempt: usize,
    /// Whether the attempt succeeded
    pub success: bool,
    /// Error if failed
    pub error: Option<String>,
    /// Duration of the attempt
    pub duration: Duration,
}

/// Fallback result
#[derive(Debug, Clone)]
pub struct FallbackResult {
    /// Successfully selected transport
    pub transport_type: TransportType,
    /// Selection result
    pub selection: SelectionResult,
    /// All attempts made
    pub attempts: Vec<FallbackAttempt>,
    /// Total time spent on fallback
    pub total_duration: Duration,
}

/// Fallback manager
pub struct FallbackManager {
    selector: BackendSelector,
    retry_config: RetryConfig,
}

impl FallbackManager {
    /// Create a new fallback manager
    pub fn new(retry_config: RetryConfig) -> Self {
        Self {
            selector: BackendSelector::new(),
            retry_config,
        }
    }

    /// Try to initialize a backend with automatic fallback
    ///
    /// This method will:
    /// 1. Try the optimal backend
    /// 2. If it fails, retry with exponential backoff
    /// 3. If all retries fail, try fallback backends
    /// 4. Return error only if all backends fail
    pub async fn try_with_fallback<F, Fut>(
        &self,
        criteria: &SelectionCriteria,
        init_fn: F,
    ) -> Result<FallbackResult>
    where
        F: Fn(TransportType) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let start_time = std::time::Instant::now();
        let mut attempts = Vec::new();

        // Get optimal backend and fallbacks
        let selection = self.selector.select_optimal(criteria)?;
        let mut backends_to_try = vec![selection.transport_type];
        backends_to_try.extend(selection.fallbacks.clone());

        info!(
            "Starting fallback sequence with {} backends: {:?}",
            backends_to_try.len(),
            backends_to_try
        );

        // Try each backend in order
        for transport_type in backends_to_try {
            debug!("Attempting backend: {}", transport_type);

            // Try with retries
            match self
                .try_backend_with_retry(transport_type, &init_fn, &mut attempts)
                .await
            {
                Ok(_) => {
                    let total_duration = start_time.elapsed();
                    info!(
                        "Successfully initialized backend {} after {} attempts in {:?}",
                        transport_type,
                        attempts.len(),
                        total_duration
                    );

                    return Ok(FallbackResult {
                        transport_type,
                        selection: selection.clone(),
                        attempts,
                        total_duration,
                    });
                }
                Err(e) => {
                    warn!(
                        "Backend {} failed after {} retries: {}",
                        transport_type, self.retry_config.max_retries, e
                    );
                    // Continue to next backend
                }
            }
        }

        // All backends failed
        let total_duration = start_time.elapsed();
        error!(
            "All backends failed after {} attempts in {:?}",
            attempts.len(),
            total_duration
        );

        Err(Error::Adapter(format!(
            "All transport backends failed. Tried {} backends with {} total attempts",
            selection.fallbacks.len() + 1,
            attempts.len()
        )))
    }

    /// Try a single backend with retry logic
    async fn try_backend_with_retry<F, Fut>(
        &self,
        transport_type: TransportType,
        init_fn: &F,
        attempts: &mut Vec<FallbackAttempt>,
    ) -> Result<()>
    where
        F: Fn(TransportType) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let mut last_error = None;

        for attempt in 1..=self.retry_config.max_retries {
            let attempt_start = std::time::Instant::now();

            debug!(
                "Attempt {}/{} for backend {}",
                attempt, self.retry_config.max_retries, transport_type
            );

            match init_fn(transport_type).await {
                Ok(_) => {
                    let duration = attempt_start.elapsed();
                    attempts.push(FallbackAttempt {
                        transport_type,
                        attempt,
                        success: true,
                        error: None,
                        duration,
                    });

                    info!(
                        "Backend {} initialized successfully on attempt {} in {:?}",
                        transport_type, attempt, duration
                    );

                    return Ok(());
                }
                Err(e) => {
                    let duration = attempt_start.elapsed();
                    let error_msg = e.to_string();

                    attempts.push(FallbackAttempt {
                        transport_type,
                        attempt,
                        success: false,
                        error: Some(error_msg.clone()),
                        duration,
                    });

                    warn!(
                        "Backend {} attempt {}/{} failed in {:?}: {}",
                        transport_type, attempt, self.retry_config.max_retries, duration, error_msg
                    );

                    last_error = Some(e);

                    // Don't sleep after last attempt
                    if attempt < self.retry_config.max_retries {
                        let backoff = self.calculate_backoff(attempt);
                        debug!("Backing off for {:?} before retry", backoff);
                        sleep(backoff).await;
                    }
                }
            }
        }

        // All retries exhausted
        Err(last_error.unwrap_or_else(|| {
            Error::Adapter(format!(
                "Backend {} failed after {} retries",
                transport_type, self.retry_config.max_retries
            ))
        }))
    }

    /// Calculate exponential backoff with optional jitter
    fn calculate_backoff(&self, attempt: usize) -> Duration {
        let base_backoff = self.retry_config.initial_backoff.as_millis() as f64
            * self.retry_config.backoff_multiplier.powi(attempt as i32 - 1);

        let backoff_ms = base_backoff.min(self.retry_config.max_backoff.as_millis() as f64);

        let final_backoff = if self.retry_config.enable_jitter {
            // Add random jitter (±25%)
            let jitter = (rand::random::<f64>() - 0.5) * 0.5;
            backoff_ms * (1.0 + jitter)
        } else {
            backoff_ms
        };

        Duration::from_millis(final_backoff as u64)
    }

    /// Get retry configuration
    pub fn retry_config(&self) -> &RetryConfig {
        &self.retry_config
    }

    /// Update retry configuration
    pub fn set_retry_config(&mut self, config: RetryConfig) {
        self.retry_config = config;
    }
}

impl Default for FallbackManager {
    fn default() -> Self {
        Self::new(RetryConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_backoff, Duration::from_millis(100));
        assert_eq!(config.max_backoff, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert!(config.enable_jitter);
    }

    #[test]
    fn test_fallback_manager_creation() {
        let config = RetryConfig::default();
        let manager = FallbackManager::new(config);
        assert_eq!(manager.retry_config().max_retries, 3);
    }

    #[test]
    fn test_backoff_calculation() {
        let config = RetryConfig {
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            enable_jitter: false,
            ..Default::default()
        };

        let manager = FallbackManager::new(config);

        // Attempt 1: 100ms
        let backoff1 = manager.calculate_backoff(1);
        assert_eq!(backoff1, Duration::from_millis(100));

        // Attempt 2: 200ms
        let backoff2 = manager.calculate_backoff(2);
        assert_eq!(backoff2, Duration::from_millis(200));

        // Attempt 3: 400ms
        let backoff3 = manager.calculate_backoff(3);
        assert_eq!(backoff3, Duration::from_millis(400));

        // Attempt 10: Should be capped at max_backoff (10s)
        let backoff10 = manager.calculate_backoff(10);
        assert_eq!(backoff10, Duration::from_secs(10));
    }

    #[test]
    fn test_backoff_with_jitter() {
        let config = RetryConfig {
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            enable_jitter: true,
            ..Default::default()
        };

        let manager = FallbackManager::new(config);

        // With jitter, backoff should vary
        let backoff1 = manager.calculate_backoff(1);
        let backoff2 = manager.calculate_backoff(1);

        // Should be within ±25% of 100ms (75-125ms)
        assert!(backoff1.as_millis() >= 75 && backoff1.as_millis() <= 125);
        assert!(backoff2.as_millis() >= 75 && backoff2.as_millis() <= 125);
    }

    #[tokio::test]
    async fn test_try_with_fallback_success_first_attempt() {
        let manager = FallbackManager::default();
        let criteria = SelectionCriteria::default();

        let result = manager
            .try_with_fallback(&criteria, |_transport_type| async { Ok(()) })
            .await;

        assert!(result.is_ok());
        let fallback_result = result.unwrap();
        assert_eq!(fallback_result.attempts.len(), 1);
        assert!(fallback_result.attempts[0].success);
    }

    #[tokio::test]
    async fn test_try_with_fallback_success_after_retry() {
        let manager = FallbackManager::new(RetryConfig {
            max_retries: 3,
            initial_backoff: Duration::from_millis(10),
            ..Default::default()
        });
        let criteria = SelectionCriteria::default();

        let attempt_count = Arc::new(AtomicUsize::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = manager
            .try_with_fallback(&criteria, move |_transport_type| {
                let count = attempt_count_clone.clone();
                async move {
                    let current = count.fetch_add(1, Ordering::SeqCst);
                    if current < 2 {
                        Err(Error::Adapter("Simulated failure".to_string()))
                    } else {
                        Ok(())
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        let fallback_result = result.unwrap();
        assert_eq!(fallback_result.attempts.len(), 3);
        assert!(!fallback_result.attempts[0].success);
        assert!(!fallback_result.attempts[1].success);
        assert!(fallback_result.attempts[2].success);
    }

    #[tokio::test]
    async fn test_try_with_fallback_all_fail() {
        let manager = FallbackManager::new(RetryConfig {
            max_retries: 2,
            initial_backoff: Duration::from_millis(10),
            ..Default::default()
        });
        let criteria = SelectionCriteria::default();

        let result = manager
            .try_with_fallback(&criteria, |_transport_type| async {
                Err(Error::Adapter("Always fails".to_string()))
            })
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_set_retry_config() {
        let mut manager = FallbackManager::default();
        
        let new_config = RetryConfig {
            max_retries: 5,
            initial_backoff: Duration::from_millis(200),
            ..Default::default()
        };

        manager.set_retry_config(new_config);
        assert_eq!(manager.retry_config().max_retries, 5);
        assert_eq!(
            manager.retry_config().initial_backoff,
            Duration::from_millis(200)
        );
    }
}

// Made with Bob