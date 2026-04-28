//! Backend Selector - Auto-selection logic for optimal transport backend
//!
//! Implements intelligent backend selection based on platform capabilities,
//! network conditions, and performance requirements.

use crate::domain::{BackendCapabilities, Platform, TransportType};
use crate::error::{Error, Result};
use tracing::{debug, info, warn};

/// Backend selection criteria
#[derive(Debug, Clone)]
pub struct SelectionCriteria {
    /// Minimum required throughput in bytes/second
    pub min_throughput_bps: Option<u64>,
    /// Maximum acceptable latency in milliseconds
    pub max_latency_ms: Option<u64>,
    /// Prefer zero-copy if available
    pub prefer_zero_copy: bool,
    /// Allow backends requiring special hardware
    pub allow_special_hardware: bool,
    /// Maximum hardware cost in USD
    pub max_hardware_cost_usd: Option<u32>,
    /// Preferred transport type (overrides auto-selection)
    pub preferred_transport: Option<TransportType>,
}

impl Default for SelectionCriteria {
    fn default() -> Self {
        Self {
            min_throughput_bps: None,
            max_latency_ms: None,
            prefer_zero_copy: true,
            allow_special_hardware: false,
            max_hardware_cost_usd: Some(500), // $500 default budget
            preferred_transport: None,
        }
    }
}

/// Backend selection result
#[derive(Debug, Clone)]
pub struct SelectionResult {
    /// Selected transport type
    pub transport_type: TransportType,
    /// Reason for selection
    pub reason: String,
    /// Backend capabilities
    pub capabilities: BackendCapabilities,
    /// Fallback options (in priority order)
    pub fallbacks: Vec<TransportType>,
}

/// Backend selector
pub struct BackendSelector {
    platform: Platform,
}

impl BackendSelector {
    /// Create a new backend selector
    pub fn new() -> Self {
        let platform = Platform::detect();
        info!(
            "Backend Selector initialized on {} {} ({})",
            platform.os, platform.os_version, platform.arch
        );
        Self { platform }
    }

    /// Select optimal backend based on criteria
    pub fn select_optimal(&self, criteria: &SelectionCriteria) -> Result<SelectionResult> {
        // If preferred transport is specified, try to use it
        if let Some(preferred) = criteria.preferred_transport {
            if let Some(result) = self.try_select_preferred(preferred, criteria) {
                return Ok(result);
            }
            warn!(
                "Preferred transport {} not available, falling back to auto-selection",
                preferred
            );
        }

        // Get all available backends
        let available = self.get_available_backends(criteria);
        if available.is_empty() {
            return Err(Error::Configuration(
                "No transport backends available on this platform".to_string(),
            ));
        }

        // Score and rank backends
        let mut scored_backends = self.score_backends(&available, criteria);
        // CORRECTNESS: scores come from arithmetic on platform metrics;
        // an unusual config (zero throughput, zero latency, divide-by-zero
        // in a future scoring component) could yield NaN, and the previous
        // `.unwrap()` would panic the entire selection path. Treat
        // unorderable pairs as Equal so selection still produces *some*
        // backend deterministically rather than killing the process.
        scored_backends.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Select best backend
        let (best_type, best_score) = scored_backends[0];
        let capabilities = BackendCapabilities::for_transport(best_type);

        // Get fallback options
        let fallbacks: Vec<TransportType> = scored_backends
            .iter()
            .skip(1)
            .map(|(t, _)| *t)
            .collect();

        let reason = self.get_selection_reason(best_type, best_score, criteria);

        info!(
            "Selected backend: {} (score: {:.2}) - {}",
            best_type, best_score, reason
        );

        Ok(SelectionResult {
            transport_type: best_type,
            reason,
            capabilities,
            fallbacks,
        })
    }

    /// Try to select a preferred transport
    fn try_select_preferred(
        &self,
        transport_type: TransportType,
        criteria: &SelectionCriteria,
    ) -> Option<SelectionResult> {
        let capabilities = BackendCapabilities::for_transport(transport_type);

        // Check if available on platform
        if !capabilities.is_available(&self.platform) {
            debug!(
                "Preferred transport {} not available on this platform",
                transport_type
            );
            return None;
        }

        // Check if meets criteria
        if !self.meets_criteria(&capabilities, criteria) {
            debug!(
                "Preferred transport {} does not meet selection criteria",
                transport_type
            );
            return None;
        }

        // Get fallback options
        let fallbacks = self.get_fallback_chain(transport_type);

        Some(SelectionResult {
            transport_type,
            reason: "User-specified preferred transport".to_string(),
            capabilities,
            fallbacks,
        })
    }

    /// Get all available backends that meet criteria
    fn get_available_backends(&self, criteria: &SelectionCriteria) -> Vec<TransportType> {
        TransportType::available_backends()
            .into_iter()
            .filter(|t| {
                let caps = BackendCapabilities::for_transport(*t);
                caps.is_available(&self.platform) && self.meets_criteria(&caps, criteria)
            })
            .collect()
    }

    /// Check if backend meets selection criteria
    fn meets_criteria(&self, caps: &BackendCapabilities, criteria: &SelectionCriteria) -> bool {
        // Check throughput requirement
        if let Some(min_throughput) = criteria.min_throughput_bps {
            if caps.max_throughput_bps < min_throughput {
                return false;
            }
        }

        // Check special hardware requirement
        if caps.requires_special_hardware && !criteria.allow_special_hardware {
            return false;
        }

        // Check hardware cost
        if let Some(max_cost) = criteria.max_hardware_cost_usd {
            if caps.hardware_cost_usd > max_cost {
                return false;
            }
        }

        true
    }

    /// Score backends based on criteria and platform
    fn score_backends(
        &self,
        backends: &[TransportType],
        criteria: &SelectionCriteria,
    ) -> Vec<(TransportType, f64)> {
        backends
            .iter()
            .map(|t| {
                let score = self.calculate_score(*t, criteria);
                (*t, score)
            })
            .collect()
    }

    /// Calculate score for a backend (0.0 - 100.0)
    fn calculate_score(&self, transport_type: TransportType, criteria: &SelectionCriteria) -> f64 {
        let caps = BackendCapabilities::for_transport(transport_type);
        let mut score = 0.0;

        // Base priority score (0-40 points)
        score += transport_type.priority() as f64 * 0.4;

        // Throughput score (0-30 points)
        let throughput_score = if let Some(min_throughput) = criteria.min_throughput_bps {
            let ratio = caps.max_throughput_bps as f64 / min_throughput as f64;
            (ratio.min(3.0) / 3.0) * 30.0 // Cap at 3x required throughput
        } else {
            // Use normalized throughput if no requirement
            (caps.max_throughput_bps as f64 / 10_000_000_000.0) * 30.0
        };
        score += throughput_score;

        // Zero-copy bonus (0-15 points)
        if criteria.prefer_zero_copy && caps.supports_zero_copy {
            score += 15.0;
        }

        // Platform optimization bonus (0-10 points)
        if self.is_optimized_for_platform(transport_type) {
            score += 10.0;
        }

        // Cost penalty (0-5 points deduction)
        if let Some(max_cost) = criteria.max_hardware_cost_usd {
            let cost_ratio = caps.hardware_cost_usd as f64 / max_cost as f64;
            if cost_ratio > 0.5 {
                score -= (cost_ratio - 0.5) * 10.0; // Penalty for expensive hardware
            }
        }

        score.max(0.0).min(100.0)
    }

    /// Check if transport is optimized for current platform
    fn is_optimized_for_platform(&self, transport_type: TransportType) -> bool {
        match transport_type {
            TransportType::IoUring => self.platform.os == "linux" && self.platform.supports_io_uring(),
            TransportType::Dpdk => self.platform.supports_dpdk() && self.platform.cpu_cores >= 8,
            TransportType::Quic => true, // Cross-platform
            TransportType::Tcp => true,  // Universal
        }
    }

    /// Get selection reason description
    fn get_selection_reason(
        &self,
        transport_type: TransportType,
        score: f64,
        criteria: &SelectionCriteria,
    ) -> String {
        let caps = BackendCapabilities::for_transport(transport_type);
        
        let mut reasons = Vec::new();

        // Primary reason
        if score >= 80.0 {
            reasons.push("Optimal performance for platform".to_string());
        } else if score >= 60.0 {
            reasons.push("Good balance of performance and compatibility".to_string());
        } else {
            reasons.push("Best available option".to_string());
        }

        // Additional factors
        if caps.supports_zero_copy && criteria.prefer_zero_copy {
            reasons.push("zero-copy support".to_string());
        }

        if caps.uses_kernel_bypass {
            reasons.push("kernel bypass".to_string());
        }

        if let Some(min_throughput) = criteria.min_throughput_bps {
            let ratio = caps.max_throughput_bps as f64 / min_throughput as f64;
            if ratio >= 2.0 {
                reasons.push(format!("{:.1}x required throughput", ratio));
            }
        }

        reasons.join(", ")
    }

    /// Get fallback chain for a transport type
    fn get_fallback_chain(&self, primary: TransportType) -> Vec<TransportType> {
        // Define fallback priority (excluding DPDK as it's not suitable for cloud)
        let priority_order = vec![
            TransportType::IoUring,
            TransportType::Quic,
            TransportType::Tcp,
        ];

        priority_order
            .into_iter()
            .filter(|t| {
                *t != primary && {
                    let caps = BackendCapabilities::for_transport(*t);
                    caps.is_available(&self.platform)
                }
            })
            .collect()
    }

    /// Get platform information
    pub fn platform(&self) -> &Platform {
        &self.platform
    }

    /// List all available backends
    pub fn list_available_backends(&self) -> Vec<(TransportType, BackendCapabilities)> {
        TransportType::available_backends()
            .into_iter()
            .filter_map(|t| {
                let caps = BackendCapabilities::for_transport(t);
                if caps.is_available(&self.platform) {
                    Some((t, caps))
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for BackendSelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selector_creation() {
        let selector = BackendSelector::new();
        assert!(!selector.platform().os.is_empty());
    }

    #[test]
    fn test_default_criteria() {
        let criteria = SelectionCriteria::default();
        assert!(criteria.prefer_zero_copy);
        assert!(!criteria.allow_special_hardware);
        assert_eq!(criteria.max_hardware_cost_usd, Some(500));
    }

    #[test]
    fn test_selection_with_default_criteria() {
        let selector = BackendSelector::new();
        let criteria = SelectionCriteria::default();
        
        let result = selector.select_optimal(&criteria);
        assert!(result.is_ok());
        
        let selection = result.unwrap();
        assert!(!selection.reason.is_empty());
        assert!(!selection.fallbacks.is_empty());
    }

    #[test]
    fn test_selection_with_high_throughput_requirement() {
        let selector = BackendSelector::new();
        let criteria = SelectionCriteria {
            min_throughput_bps: Some(5_000_000_000), // 5 GB/s
            ..Default::default()
        };
        
        let result = selector.select_optimal(&criteria);
        if let Ok(selection) = result {
            // Should select io_uring or DPDK if available
            assert!(matches!(
                selection.transport_type,
                TransportType::IoUring | TransportType::Dpdk
            ));
        }
    }

    #[test]
    fn test_preferred_transport() {
        let selector = BackendSelector::new();
        let criteria = SelectionCriteria {
            preferred_transport: Some(TransportType::Tcp),
            ..Default::default()
        };
        
        let result = selector.select_optimal(&criteria);
        assert!(result.is_ok());
        
        let selection = result.unwrap();
        assert_eq!(selection.transport_type, TransportType::Tcp);
    }

    #[test]
    fn test_list_available_backends() {
        let selector = BackendSelector::new();
        let available = selector.list_available_backends();
        
        // At least TCP should be available
        assert!(!available.is_empty());
        assert!(available.iter().any(|(t, _)| *t == TransportType::Tcp));
    }

    #[test]
    fn test_fallback_chain() {
        let selector = BackendSelector::new();
        let fallbacks = selector.get_fallback_chain(TransportType::IoUring);
        
        // Should not include the primary transport
        assert!(!fallbacks.contains(&TransportType::IoUring));
        
        // Should be ordered by priority
        if fallbacks.len() >= 2 {
            let first_caps = BackendCapabilities::for_transport(fallbacks[0]);
            let second_caps = BackendCapabilities::for_transport(fallbacks[1]);
            assert!(first_caps.max_throughput_bps >= second_caps.max_throughput_bps);
        }
    }

    #[test]
    fn test_score_calculation() {
        let selector = BackendSelector::new();
        let criteria = SelectionCriteria::default();
        
        let io_uring_score = selector.calculate_score(TransportType::IoUring, &criteria);
        let tcp_score = selector.calculate_score(TransportType::Tcp, &criteria);
        
        // io_uring should score higher than TCP
        assert!(io_uring_score > tcp_score);
    }

    #[test]
    fn test_meets_criteria() {
        let selector = BackendSelector::new();
        
        // Test throughput requirement
        let criteria = SelectionCriteria {
            min_throughput_bps: Some(5_000_000_000), // 5 GB/s
            ..Default::default()
        };
        
        let io_uring_caps = BackendCapabilities::for_transport(TransportType::IoUring);
        let tcp_caps = BackendCapabilities::for_transport(TransportType::Tcp);
        
        assert!(selector.meets_criteria(&io_uring_caps, &criteria));
        assert!(!selector.meets_criteria(&tcp_caps, &criteria));
    }

    #[test]
    fn test_special_hardware_filtering() {
        let selector = BackendSelector::new();
        
        let criteria = SelectionCriteria {
            allow_special_hardware: false,
            ..Default::default()
        };
        
        let dpdk_caps = BackendCapabilities::for_transport(TransportType::Dpdk);
        // Should not meet criteria when special hardware is not allowed
        assert!(!selector.meets_criteria(&dpdk_caps, &criteria));
        
        let criteria_with_hw = SelectionCriteria {
            allow_special_hardware: true,
            ..Default::default()
        };
        
        // When special hardware is allowed, it should meet criteria
        // (assuming other requirements like platform support are met)
        // Note: This may still fail if DPDK is not available on the platform
        let meets_hw_criteria = selector.meets_criteria(&dpdk_caps, &criteria_with_hw);
        
        // If DPDK is available on platform, it should meet criteria
        // If not available, it won't meet criteria regardless of allow_special_hardware
        if dpdk_caps.is_available(&selector.platform) {
            assert!(meets_hw_criteria);
        } else {
            // DPDK not available on this platform, so it won't meet criteria
            assert!(!meets_hw_criteria);
        }
    }
}

// Made with Bob