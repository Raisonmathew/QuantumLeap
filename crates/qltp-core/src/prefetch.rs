//! Predictive pre-fetching for QLTP
//!
//! This module implements intelligent pre-fetching of data chunks based on
//! access patterns and predictions. It uses historical data and heuristics
//! to anticipate which chunks will be needed next.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Pre-fetch configuration
#[derive(Debug, Clone)]
pub struct PrefetchConfig {
    /// Maximum number of chunks to pre-fetch
    pub max_prefetch_chunks: usize,
    /// Minimum confidence threshold (0.0 - 1.0)
    pub min_confidence: f64,
    /// Enable sequential pre-fetching
    pub enable_sequential: bool,
    /// Enable pattern-based pre-fetching
    pub enable_pattern: bool,
    /// History size for pattern detection
    pub history_size: usize,
    /// Pre-fetch window size
    pub window_size: usize,
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            max_prefetch_chunks: 10,
            min_confidence: 0.7,
            enable_sequential: true,
            enable_pattern: true,
            history_size: 100,
            window_size: 5,
        }
    }
}

/// Access pattern types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPattern {
    /// Sequential access (1, 2, 3, 4, ...)
    Sequential,
    /// Strided access (1, 3, 5, 7, ...)
    Strided(usize),
    /// Random access
    Random,
    /// Repeated access to same chunks
    Repeated,
}

/// Pre-fetch prediction
#[derive(Debug, Clone)]
pub struct Prediction {
    /// Chunk ID to pre-fetch
    pub chunk_id: u64,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Pattern that generated this prediction
    pub pattern: AccessPattern,
}

/// Pre-fetch statistics
#[derive(Debug, Clone, Default)]
pub struct PrefetchStats {
    /// Total predictions made
    pub predictions_made: u64,
    /// Successful predictions (chunk was actually accessed)
    pub predictions_hit: u64,
    /// Failed predictions (chunk was not accessed)
    pub predictions_miss: u64,
    /// Chunks pre-fetched
    pub chunks_prefetched: u64,
    /// Bytes pre-fetched
    pub bytes_prefetched: u64,
    /// Time saved by pre-fetching (estimated, in milliseconds)
    pub time_saved_ms: u64,
}

impl PrefetchStats {
    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.predictions_made == 0 {
            0.0
        } else {
            self.predictions_hit as f64 / self.predictions_made as f64
        }
    }

    /// Calculate efficiency
    pub fn efficiency(&self) -> f64 {
        if self.chunks_prefetched == 0 {
            0.0
        } else {
            self.predictions_hit as f64 / self.chunks_prefetched as f64
        }
    }
}

/// Predictive pre-fetcher
pub struct Prefetcher {
    config: PrefetchConfig,
    access_history: Arc<Mutex<VecDeque<u64>>>,
    pattern_cache: Arc<Mutex<HashMap<Vec<u64>, Vec<u64>>>>,
    stats: Arc<Mutex<PrefetchStats>>,
}

impl Prefetcher {
    /// Create a new pre-fetcher
    pub fn new(config: PrefetchConfig) -> Self {
        info!("Creating pre-fetcher with config: {:?}", config);
        Self {
            config,
            access_history: Arc::new(Mutex::new(VecDeque::new())),
            pattern_cache: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(PrefetchStats::default())),
        }
    }

    /// Record a chunk access
    pub async fn record_access(&self, chunk_id: u64) {
        let mut history = self.access_history.lock().await;
        
        // Add to history
        history.push_back(chunk_id);
        
        // Limit history size
        while history.len() > self.config.history_size {
            history.pop_front();
        }
        
        debug!("Recorded access to chunk {}, history size: {}", chunk_id, history.len());
    }

    /// Predict next chunks to pre-fetch
    pub async fn predict_next(&self, current_chunk: u64) -> Vec<Prediction> {
        let mut predictions = Vec::new();

        // Sequential prediction
        if self.config.enable_sequential {
            if let Some(seq_pred) = self.predict_sequential(current_chunk).await {
                predictions.extend(seq_pred);
            }
        }

        // Pattern-based prediction
        if self.config.enable_pattern {
            if let Some(pattern_pred) = self.predict_pattern(current_chunk).await {
                predictions.extend(pattern_pred);
            }
        }

        // Filter by confidence and limit count
        predictions.retain(|p| p.confidence >= self.config.min_confidence);
        predictions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        predictions.truncate(self.config.max_prefetch_chunks);

        // Update stats
        {
            let mut stats = self.stats.lock().await;
            stats.predictions_made += predictions.len() as u64;
        }

        debug!("Generated {} predictions for chunk {}", predictions.len(), current_chunk);
        predictions
    }

    /// Predict sequential access
    async fn predict_sequential(&self, current_chunk: u64) -> Option<Vec<Prediction>> {
        let history = self.access_history.lock().await;
        
        if history.len() < 3 {
            return None;
        }

        // Check if recent accesses are sequential
        let recent: Vec<u64> = history.iter().rev().take(5).copied().collect();
        let is_sequential = recent.windows(2).all(|w| w[0] == w[1] + 1);

        if !is_sequential {
            return None;
        }

        // Predict next sequential chunks
        let mut predictions = Vec::new();
        for i in 1..=self.config.window_size {
            predictions.push(Prediction {
                chunk_id: current_chunk + i as u64,
                confidence: 0.9 - (i as f64 * 0.1), // Decreasing confidence
                pattern: AccessPattern::Sequential,
            });
        }

        Some(predictions)
    }

    /// Predict based on historical patterns
    async fn predict_pattern(&self, current_chunk: u64) -> Option<Vec<Prediction>> {
        let history = self.access_history.lock().await;
        
        if history.len() < self.config.window_size {
            return None;
        }

        // Get recent pattern
        let pattern: Vec<u64> = history.iter()
            .rev()
            .take(self.config.window_size)
            .copied()
            .collect();

        // Check pattern cache
        let cache = self.pattern_cache.lock().await;
        if let Some(next_chunks) = cache.get(&pattern) {
            let predictions: Vec<Prediction> = next_chunks.iter()
                .take(self.config.max_prefetch_chunks)
                .map(|&chunk_id| Prediction {
                    chunk_id,
                    confidence: 0.8,
                    pattern: AccessPattern::Random,
                })
                .collect();
            
            return Some(predictions);
        }

        // Detect stride pattern
        if let Some(stride) = self.detect_stride(&pattern) {
            let mut predictions = Vec::new();
            for i in 1..=self.config.window_size {
                predictions.push(Prediction {
                    chunk_id: current_chunk + (stride * i) as u64,
                    confidence: 0.85,
                    pattern: AccessPattern::Strided(stride),
                });
            }
            return Some(predictions);
        }

        None
    }

    /// Detect stride in access pattern
    fn detect_stride(&self, pattern: &[u64]) -> Option<usize> {
        if pattern.len() < 3 {
            return None;
        }

        // Calculate differences
        let diffs: Vec<i64> = pattern.windows(2)
            .map(|w| w[1] as i64 - w[0] as i64)
            .collect();

        // Check if all differences are the same
        if diffs.windows(2).all(|w| w[0] == w[1]) && diffs[0] > 0 {
            Some(diffs[0] as usize)
        } else {
            None
        }
    }

    /// Update pattern cache with observed sequence
    pub async fn update_pattern_cache(&self, pattern: Vec<u64>, next_chunk: u64) {
        let mut cache = self.pattern_cache.lock().await;
        
        cache.entry(pattern)
            .or_insert_with(Vec::new)
            .push(next_chunk);
        
        // Limit cache size
        if cache.len() > 1000 {
            // Remove oldest entries (simple FIFO)
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }
    }

    /// Mark a prediction as hit (chunk was accessed)
    pub async fn mark_hit(&self, chunk_id: u64) {
        let mut stats = self.stats.lock().await;
        stats.predictions_hit += 1;
        
        debug!("Prediction hit for chunk {}", chunk_id);
    }

    /// Mark a prediction as miss (chunk was not accessed)
    pub async fn mark_miss(&self, chunk_id: u64) {
        let mut stats = self.stats.lock().await;
        stats.predictions_miss += 1;
        
        debug!("Prediction miss for chunk {}", chunk_id);
    }

    /// Record successful pre-fetch
    pub async fn record_prefetch(&self, chunk_id: u64, bytes: u64, time_saved_ms: u64) {
        let mut stats = self.stats.lock().await;
        stats.chunks_prefetched += 1;
        stats.bytes_prefetched += bytes;
        stats.time_saved_ms += time_saved_ms;
        
        debug!("Pre-fetched chunk {} ({} bytes, saved {} ms)", chunk_id, bytes, time_saved_ms);
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> PrefetchStats {
        self.stats.lock().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.lock().await;
        *stats = PrefetchStats::default();
    }

    /// Clear history and cache
    pub async fn clear(&self) {
        let mut history = self.access_history.lock().await;
        history.clear();
        
        let mut cache = self.pattern_cache.lock().await;
        cache.clear();
        
        info!("Cleared pre-fetch history and cache");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prefetch_config_default() {
        let config = PrefetchConfig::default();
        assert_eq!(config.max_prefetch_chunks, 10);
        assert_eq!(config.min_confidence, 0.7);
        assert!(config.enable_sequential);
        assert!(config.enable_pattern);
    }

    #[tokio::test]
    async fn test_prefetcher_creation() {
        let config = PrefetchConfig::default();
        let prefetcher = Prefetcher::new(config);
        
        let stats = prefetcher.get_stats().await;
        assert_eq!(stats.predictions_made, 0);
        assert_eq!(stats.predictions_hit, 0);
    }

    #[tokio::test]
    async fn test_record_access() {
        let config = PrefetchConfig::default();
        let prefetcher = Prefetcher::new(config);
        
        prefetcher.record_access(1).await;
        prefetcher.record_access(2).await;
        prefetcher.record_access(3).await;
        
        let history = prefetcher.access_history.lock().await;
        assert_eq!(history.len(), 3);
    }

    #[tokio::test]
    async fn test_sequential_prediction() {
        let config = PrefetchConfig::default();
        let prefetcher = Prefetcher::new(config);
        
        // Create sequential access pattern
        for i in 1..=5 {
            prefetcher.record_access(i).await;
        }
        
        // Predict next chunks
        let predictions = prefetcher.predict_next(5).await;
        
        assert!(!predictions.is_empty());
        assert_eq!(predictions[0].chunk_id, 6);
        assert_eq!(predictions[0].pattern, AccessPattern::Sequential);
        assert!(predictions[0].confidence > 0.7);
    }

    #[tokio::test]
    async fn test_stride_detection() {
        let config = PrefetchConfig::default();
        let prefetcher = Prefetcher::new(config);
        
        // Create strided access pattern (stride = 2)
        for i in (1..=10).step_by(2) {
            prefetcher.record_access(i).await;
        }
        
        let pattern = vec![1, 3, 5, 7, 9];
        let stride = prefetcher.detect_stride(&pattern);
        
        assert_eq!(stride, Some(2));
    }

    #[tokio::test]
    async fn test_pattern_cache() {
        let config = PrefetchConfig::default();
        let prefetcher = Prefetcher::new(config);
        
        let pattern = vec![1, 2, 3];
        prefetcher.update_pattern_cache(pattern.clone(), 4).await;
        
        let cache = prefetcher.pattern_cache.lock().await;
        assert!(cache.contains_key(&pattern));
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let config = PrefetchConfig::default();
        let prefetcher = Prefetcher::new(config);
        
        prefetcher.mark_hit(1).await;
        prefetcher.mark_hit(2).await;
        prefetcher.mark_miss(3).await;
        
        let stats = prefetcher.get_stats().await;
        assert_eq!(stats.predictions_hit, 2);
        assert_eq!(stats.predictions_miss, 1);
    }

    #[tokio::test]
    async fn test_hit_rate_calculation() {
        let mut stats = PrefetchStats::default();
        stats.predictions_made = 10;
        stats.predictions_hit = 8;
        
        assert_eq!(stats.hit_rate(), 0.8);
    }

    #[tokio::test]
    async fn test_efficiency_calculation() {
        let mut stats = PrefetchStats::default();
        stats.chunks_prefetched = 10;
        stats.predictions_hit = 7;
        
        assert_eq!(stats.efficiency(), 0.7);
    }

    #[tokio::test]
    async fn test_prefetch_recording() {
        let config = PrefetchConfig::default();
        let prefetcher = Prefetcher::new(config);
        
        prefetcher.record_prefetch(1, 1024, 10).await;
        prefetcher.record_prefetch(2, 2048, 15).await;
        
        let stats = prefetcher.get_stats().await;
        assert_eq!(stats.chunks_prefetched, 2);
        assert_eq!(stats.bytes_prefetched, 3072);
        assert_eq!(stats.time_saved_ms, 25);
    }

    #[tokio::test]
    async fn test_clear_history() {
        let config = PrefetchConfig::default();
        let prefetcher = Prefetcher::new(config);
        
        for i in 1..=10 {
            prefetcher.record_access(i).await;
        }
        
        prefetcher.clear().await;
        
        let history = prefetcher.access_history.lock().await;
        assert_eq!(history.len(), 0);
    }

    #[tokio::test]
    async fn test_confidence_filtering() {
        let config = PrefetchConfig {
            min_confidence: 0.8,
            ..Default::default()
        };
        let prefetcher = Prefetcher::new(config);
        
        // Create sequential pattern
        for i in 1..=5 {
            prefetcher.record_access(i).await;
        }
        
        let predictions = prefetcher.predict_next(5).await;
        
        // All predictions should have confidence >= 0.8
        for pred in predictions {
            assert!(pred.confidence >= 0.8);
        }
    }
}

// Made with Bob
