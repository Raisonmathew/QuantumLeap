//! Buffer Pool Implementation
//!
//! Phase 6.2.3: Efficient buffer pooling to reduce memory allocations
//! 
//! Features:
//! - Pre-allocated buffer pool
//! - Automatic buffer reuse
//! - Dynamic pool growth up to max limit
//! - Thread-safe buffer management
//! - Optimized for jumbo frames (9000 bytes)

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use crate::error::{Error, Result};

/// Default buffer size optimized for jumbo frames
pub const DEFAULT_BUFFER_SIZE: usize = 9000;

/// Default initial buffer count
pub const DEFAULT_INITIAL_BUFFERS: usize = 64;

/// Default maximum buffer count
pub const DEFAULT_MAX_BUFFERS: usize = 256;

/// Buffer handle returned when acquiring a buffer
pub struct BufferHandle {
    buffer: Vec<u8>,
    index: usize,
    pool: Arc<Mutex<BufferPoolInner>>,
}

impl BufferHandle {
    /// Get a mutable reference to the buffer
    pub fn as_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    /// Get an immutable reference to the buffer
    pub fn as_ref(&self) -> &Vec<u8> {
        &self.buffer
    }

    /// Get the buffer size
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Clear the buffer contents
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Resize the buffer
    pub fn resize(&mut self, new_len: usize, value: u8) {
        self.buffer.resize(new_len, value);
    }
}

impl Drop for BufferHandle {
    fn drop(&mut self) {
        // Return buffer to pool when handle is dropped
        if let Ok(mut pool) = self.pool.lock() {
            pool.return_buffer(self.index, std::mem::take(&mut self.buffer));
        }
    }
}

/// Inner buffer pool implementation
struct BufferPoolInner {
    /// All allocated buffers
    buffers: Vec<Option<Vec<u8>>>,
    /// Indices of available buffers
    available: VecDeque<usize>,
    /// Size of each buffer
    buffer_size: usize,
    /// Maximum number of buffers
    max_buffers: usize,
    /// Statistics
    stats: PoolStats,
}

/// Buffer pool statistics
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// Total buffers allocated
    pub total_allocated: usize,
    /// Currently in use
    pub in_use: usize,
    /// Total acquisitions
    pub total_acquisitions: u64,
    /// Total releases
    pub total_releases: u64,
    /// Failed acquisitions (pool exhausted)
    pub failed_acquisitions: u64,
}

impl BufferPoolInner {
    fn new(buffer_size: usize, initial_count: usize, max_buffers: usize) -> Self {
        let mut buffers = Vec::with_capacity(initial_count);
        let mut available = VecDeque::with_capacity(initial_count);

        // Pre-allocate initial buffers
        for i in 0..initial_count {
            buffers.push(Some(vec![0u8; buffer_size]));
            available.push_back(i);
        }

        Self {
            buffers,
            available,
            buffer_size,
            max_buffers,
            stats: PoolStats {
                total_allocated: initial_count,
                in_use: 0,
                total_acquisitions: 0,
                total_releases: 0,
                failed_acquisitions: 0,
            },
        }
    }

    fn acquire_buffer(&mut self) -> Option<(usize, Vec<u8>)> {
        self.stats.total_acquisitions += 1;

        // Try to get an available buffer
        if let Some(idx) = self.available.pop_front() {
            if let Some(buffer) = self.buffers[idx].take() {
                self.stats.in_use += 1;
                return Some((idx, buffer));
            }
        }

        // If no available buffers, try to allocate a new one
        if self.buffers.len() < self.max_buffers {
            let idx = self.buffers.len();
            let buffer = vec![0u8; self.buffer_size];
            self.buffers.push(None);
            self.stats.total_allocated += 1;
            self.stats.in_use += 1;
            return Some((idx, buffer));
        }

        // Pool exhausted
        self.stats.failed_acquisitions += 1;
        None
    }

    fn return_buffer(&mut self, index: usize, mut buffer: Vec<u8>) {
        self.stats.total_releases += 1;
        self.stats.in_use = self.stats.in_use.saturating_sub(1);

        // Reset buffer to original size
        buffer.clear();
        buffer.resize(self.buffer_size, 0);

        // Return buffer to pool
        if index < self.buffers.len() {
            self.buffers[index] = Some(buffer);
            self.available.push_back(index);
        }
    }

    fn stats(&self) -> PoolStats {
        self.stats.clone()
    }
}

/// Thread-safe buffer pool
#[derive(Clone)]
pub struct BufferPool {
    inner: Arc<Mutex<BufferPoolInner>>,
}

impl BufferPool {
    /// Create a new buffer pool with default settings
    pub fn new() -> Self {
        Self::with_config(
            DEFAULT_BUFFER_SIZE,
            DEFAULT_INITIAL_BUFFERS,
            DEFAULT_MAX_BUFFERS,
        )
    }

    /// Create a new buffer pool with custom configuration
    pub fn with_config(
        buffer_size: usize,
        initial_count: usize,
        max_buffers: usize,
    ) -> Self {
        let inner = BufferPoolInner::new(buffer_size, initial_count, max_buffers);
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// Acquire a buffer from the pool
    pub fn acquire(&self) -> Result<BufferHandle> {
        let mut pool = self.inner.lock()
            .map_err(|_| Error::Domain("Failed to lock buffer pool".to_string()))?;

        let (index, buffer) = pool.acquire_buffer()
            .ok_or_else(|| Error::Domain("Buffer pool exhausted".to_string()))?;

        Ok(BufferHandle {
            buffer,
            index,
            pool: Arc::clone(&self.inner),
        })
    }

    /// Get pool statistics
    pub fn stats(&self) -> Result<PoolStats> {
        let pool = self.inner.lock()
            .map_err(|_| Error::Domain("Failed to lock buffer pool".to_string()))?;
        Ok(pool.stats())
    }

    /// Get the buffer size
    pub fn buffer_size(&self) -> Result<usize> {
        let pool = self.inner.lock()
            .map_err(|_| Error::Domain("Failed to lock buffer pool".to_string()))?;
        Ok(pool.buffer_size)
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_creation() {
        let pool = BufferPool::new();
        let stats = pool.stats().unwrap();
        
        assert_eq!(stats.total_allocated, DEFAULT_INITIAL_BUFFERS);
        assert_eq!(stats.in_use, 0);
    }

    #[test]
    fn test_buffer_acquisition() {
        let pool = BufferPool::new();
        let buffer = pool.acquire().unwrap();
        
        assert_eq!(buffer.len(), DEFAULT_BUFFER_SIZE);
        
        let stats = pool.stats().unwrap();
        assert_eq!(stats.in_use, 1);
        assert_eq!(stats.total_acquisitions, 1);
    }

    #[test]
    fn test_buffer_release() {
        let pool = BufferPool::new();
        
        {
            let _buffer = pool.acquire().unwrap();
            let stats = pool.stats().unwrap();
            assert_eq!(stats.in_use, 1);
        }
        
        // Buffer should be released when dropped
        let stats = pool.stats().unwrap();
        assert_eq!(stats.in_use, 0);
        assert_eq!(stats.total_releases, 1);
    }

    #[test]
    fn test_buffer_reuse() {
        let pool = BufferPool::new();
        
        // Acquire and release
        {
            let _buffer = pool.acquire().unwrap();
        }
        
        // Acquire again - should reuse the same buffer
        let _buffer = pool.acquire().unwrap();
        
        let stats = pool.stats().unwrap();
        assert_eq!(stats.total_allocated, DEFAULT_INITIAL_BUFFERS);
        assert_eq!(stats.total_acquisitions, 2);
    }

    #[test]
    fn test_pool_growth() {
        let pool = BufferPool::with_config(1024, 2, 10);
        
        // Acquire more buffers than initial count
        let _b1 = pool.acquire().unwrap();
        let _b2 = pool.acquire().unwrap();
        let _b3 = pool.acquire().unwrap();
        
        let stats = pool.stats().unwrap();
        assert_eq!(stats.total_allocated, 3);
        assert_eq!(stats.in_use, 3);
    }

    #[test]
    fn test_pool_exhaustion() {
        let pool = BufferPool::with_config(1024, 2, 2);
        
        let _b1 = pool.acquire().unwrap();
        let _b2 = pool.acquire().unwrap();
        
        // Pool should be exhausted
        let result = pool.acquire();
        assert!(result.is_err());
        
        let stats = pool.stats().unwrap();
        assert_eq!(stats.failed_acquisitions, 1);
    }

    #[test]
    fn test_buffer_handle_operations() {
        let pool = BufferPool::new();
        let mut buffer = pool.acquire().unwrap();
        
        // Test clear
        buffer.clear();
        assert_eq!(buffer.len(), 0);
        
        // Test resize
        buffer.resize(100, 42);
        assert_eq!(buffer.len(), 100);
        assert_eq!(buffer.as_ref()[0], 42);
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;
        
        let pool = BufferPool::new();
        let mut handles = vec![];
        
        // Spawn multiple threads acquiring buffers
        for _ in 0..10 {
            let pool_clone = pool.clone();
            let handle = thread::spawn(move || {
                let _buffer = pool_clone.acquire().unwrap();
                thread::sleep(std::time::Duration::from_millis(10));
            });
            handles.push(handle);
        }
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
        
        // All buffers should be released
        let stats = pool.stats().unwrap();
        assert_eq!(stats.in_use, 0);
    }
}

// Made with Bob
