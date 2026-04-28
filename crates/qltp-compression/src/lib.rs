//! Compression layer for QLTP
//!
//! Provides high-speed compression using LZ4 and Zstandard algorithms.

use std::io::{Read, Write};
use tracing::{debug, instrument};

/// Error type for compression operations
pub type Error = anyhow::Error;

/// Result type for compression operations
pub type Result<T> = std::result::Result<T, Error>;

/// Default maximum decompressed size (256 MiB).
///
/// Chosen to comfortably fit a single QLTP transfer chunk (which is at most
/// a few MiB after content-defined chunking) plus a wide safety margin,
/// while staying small enough that an attacker cannot exhaust host RAM by
/// streaming a single crafted frame. Callers handling explicitly larger
/// trusted blobs should call [`decompress_with_limit`] with a custom cap.
pub const DEFAULT_MAX_DECOMPRESSED_SIZE: usize = 256 * 1024 * 1024;

/// Create a compression error
fn compression_error(msg: String) -> Error {
    anyhow::anyhow!("{}", msg)
}

/// Create a decompression error
fn decompression_error(msg: String) -> Error {
    anyhow::anyhow!("{}", msg)
}

/// Compression algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// LZ4 - Fast compression (500+ MB/s)
    Lz4,
    /// Zstandard - Balanced compression/ratio
    Zstd,
    /// No compression
    None,
}

impl Algorithm {
    /// Get the name of the algorithm
    pub fn name(&self) -> &'static str {
        match self {
            Algorithm::Lz4 => "LZ4",
            Algorithm::Zstd => "Zstandard",
            Algorithm::None => "None",
        }
    }
}

/// Compression level (1-22 for Zstd, ignored for LZ4)
#[derive(Debug, Clone, Copy)]
pub struct CompressionLevel(i32);

impl CompressionLevel {
    /// Fast compression (level 1)
    pub const FAST: Self = Self(1);
    /// Default compression (level 3)
    pub const DEFAULT: Self = Self(3);
    /// Best compression (level 22)
    pub const BEST: Self = Self(22);

    /// Create a new compression level
    pub fn new(level: i32) -> Result<Self> {
        if !(1..=22).contains(&level) {
            return Err(compression_error(format!(
                "Invalid compression level: {} (must be 1-22)",
                level
            )));
        }
        Ok(Self(level))
    }

    /// Get the level value
    pub fn value(&self) -> i32 {
        self.0
    }
}

impl Default for CompressionLevel {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Compress data using the specified algorithm
#[instrument(skip(data))]
pub fn compress(data: &[u8], algorithm: Algorithm, level: CompressionLevel) -> Result<Vec<u8>> {
    debug!(
        "Compressing {} bytes with {} (level {})",
        data.len(),
        algorithm.name(),
        level.value()
    );

    let compressed = match algorithm {
        Algorithm::Lz4 => compress_lz4(data)?,
        Algorithm::Zstd => compress_zstd(data, level)?,
        Algorithm::None => data.to_vec(),
    };

    let ratio = data.len() as f64 / compressed.len() as f64;
    debug!(
        "Compressed {} bytes -> {} bytes (ratio: {:.2}x)",
        data.len(),
        compressed.len(),
        ratio
    );

    Ok(compressed)
}

/// Decompress data using the specified algorithm.
///
/// Uses the workspace default decompression cap of [`DEFAULT_MAX_DECOMPRESSED_SIZE`]
/// to defend against compression-bomb attacks. Callers that need a different
/// limit (or, for trusted internal data, a larger one) should use
/// [`decompress_with_limit`] directly.
#[instrument(skip(data))]
pub fn decompress(data: &[u8], algorithm: Algorithm) -> Result<Vec<u8>> {
    decompress_with_limit(data, algorithm, DEFAULT_MAX_DECOMPRESSED_SIZE)
}

/// Decompress data with an explicit maximum-output cap.
///
/// SECURITY (CWE-409, decompression bomb): Both LZ4 and Zstd format
/// streams can encode a tiny payload that expands to gigabytes when
/// decompressed. Reading without a hard upper bound (`read_to_end` /
/// `decode_all`) is exploitable for DoS by any peer that controls the
/// compressed bytes.  We wrap each decoder in `.take(max_output as u64 + 1)`,
/// which causes the read to stop one byte past the limit; if that final
/// byte was actually consumed we know the producer wanted more than the
/// cap and reject the stream with [`Error`].
pub fn decompress_with_limit(
    data: &[u8],
    algorithm: Algorithm,
    max_output: usize,
) -> Result<Vec<u8>> {
    debug!(
        "Decompressing {} bytes with {} (cap {} bytes)",
        data.len(),
        algorithm.name(),
        max_output
    );

    let decompressed = match algorithm {
        Algorithm::Lz4 => decompress_lz4(data, max_output)?,
        Algorithm::Zstd => decompress_zstd(data, max_output)?,
        Algorithm::None => {
            if data.len() > max_output {
                return Err(decompression_error(format!(
                    "Decompressed size exceeds limit: {} > {}",
                    data.len(),
                    max_output
                )));
            }
            data.to_vec()
        }
    };

    debug!(
        "Decompressed {} bytes -> {} bytes",
        data.len(),
        decompressed.len()
    );

    Ok(decompressed)
}

/// Compress data using LZ4
fn compress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = lz4::EncoderBuilder::new()
        .level(4) // Fast compression
        .build(Vec::new())
        .map_err(|e| compression_error(format!("LZ4 encoder error: {}", e)))?;

    encoder
        .write_all(data)
        .map_err(|e| compression_error(format!("LZ4 write error: {}", e)))?;

    let (compressed, result): (Vec<u8>, std::io::Result<()>) = encoder.finish();
    result.map_err(|e| compression_error(format!("LZ4 finish error: {}", e)))?;

    Ok(compressed)
}

/// Decompress data using LZ4, capped at `max_output` bytes.
fn decompress_lz4(data: &[u8], max_output: usize) -> Result<Vec<u8>> {
    let decoder = lz4::Decoder::new(data)
        .map_err(|e| decompression_error(format!("LZ4 decoder error: {}", e)))?;

    // `take(max_output + 1)` lets us detect when the producer wants to write
    // more than the cap: we read up to one extra byte; if it materialised,
    // the stream was over-budget and we reject.
    let mut limited = decoder.take(max_output as u64 + 1);
    let mut decompressed = Vec::with_capacity(data.len().min(max_output));
    limited
        .read_to_end(&mut decompressed)
        .map_err(|e| decompression_error(format!("LZ4 read error: {}", e)))?;

    if decompressed.len() > max_output {
        return Err(decompression_error(format!(
            "LZ4 decompressed size exceeds limit: > {} bytes",
            max_output
        )));
    }

    Ok(decompressed)
}

/// Compress data using Zstandard
fn compress_zstd(data: &[u8], level: CompressionLevel) -> Result<Vec<u8>> {
    zstd::encode_all(data, level.value())
        .map_err(|e| compression_error(format!("Zstd compression error: {}", e)))
}

/// Decompress data using Zstandard, capped at `max_output` bytes.
fn decompress_zstd(data: &[u8], max_output: usize) -> Result<Vec<u8>> {
    // Use the streaming Decoder so we can apply a hard read cap. Same
    // `take(max_output + 1)` trick as LZ4 to detect overflow.
    let decoder = zstd::stream::Decoder::new(data)
        .map_err(|e| decompression_error(format!("Zstd decoder error: {}", e)))?;

    let mut limited = decoder.take(max_output as u64 + 1);
    let mut decompressed = Vec::with_capacity(data.len().min(max_output));
    limited
        .read_to_end(&mut decompressed)
        .map_err(|e| decompression_error(format!("Zstd read error: {}", e)))?;

    if decompressed.len() > max_output {
        return Err(decompression_error(format!(
            "Zstd decompressed size exceeds limit: > {} bytes",
            max_output
        )));
    }

    Ok(decompressed)
}

/// Calculate compression ratio
pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f64 {
    if compressed_size == 0 {
        return 0.0;
    }
    original_size as f64 / compressed_size as f64
}

/// Estimate if compression is worthwhile.
///
/// `min_ratio` is the minimum estimated compression ratio
/// (`original / compressed`) required for the function to return `true`.
/// We approximate the achievable ratio from byte-frequency entropy on a
/// 1 KiB sample.
pub fn should_compress(data: &[u8], min_size: usize, min_ratio: f64) -> bool {
    if data.len() < min_size {
        return false;
    }

    // Quick entropy check: if data is already compressed/encrypted, skip
    // Count unique bytes in first 1KB
    let sample_size = data.len().min(1024);
    let mut seen = [false; 256];
    let mut unique_count = 0;

    for &byte in &data[..sample_size] {
        if !seen[byte as usize] {
            seen[byte as usize] = true;
            unique_count += 1;
        }
    }

    // If entropy is high (many unique bytes), compression likely won't help much.
    // Map `min_ratio` to a maximum acceptable entropy: estimated ratio
    // ~= 1 / entropy_ratio, so accept when entropy_ratio < 1 / min_ratio.
    // Clamp at 0.9 so near-random data is never compressed regardless of
    // a permissive min_ratio.
    let entropy_ratio = unique_count as f64 / 256.0;
    let entropy_ceiling = if min_ratio > 1.0 {
        (1.0 / min_ratio).min(0.9)
    } else {
        0.9
    };
    entropy_ratio < entropy_ceiling
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz4_compression() {
        let data = b"Hello, World! This is a test of LZ4 compression. ".repeat(100);
        
        let compressed = compress(&data, Algorithm::Lz4, CompressionLevel::DEFAULT).unwrap();
        assert!(compressed.len() < data.len());

        let decompressed = decompress(&compressed, Algorithm::Lz4).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_zstd_compression() {
        let data = b"Hello, World! This is a test of Zstandard compression. ".repeat(100);
        
        let compressed = compress(&data, Algorithm::Zstd, CompressionLevel::DEFAULT).unwrap();
        assert!(compressed.len() < data.len());

        let decompressed = decompress(&compressed, Algorithm::Zstd).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_no_compression() {
        let data = b"Hello, World!";
        
        let compressed = compress(data, Algorithm::None, CompressionLevel::DEFAULT).unwrap();
        assert_eq!(data, compressed.as_slice());

        let decompressed = decompress(&compressed, Algorithm::None).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_compression_ratio() {
        assert_eq!(compression_ratio(1000, 500), 2.0);
        assert_eq!(compression_ratio(1000, 250), 4.0);
        assert_eq!(compression_ratio(1000, 1000), 1.0);
    }

    #[test]
    fn test_should_compress() {
        // Small data should not be compressed
        let small_data = b"Hello";
        assert!(!should_compress(small_data, 1024, 1.5));

        // Repetitive data should be compressed
        let repetitive_data = b"AAAAAAAAAA".repeat(200);
        assert!(should_compress(&repetitive_data, 1024, 1.5));

        // Random data (high entropy) should not be compressed
        let random_data: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();
        assert!(!should_compress(&random_data, 1024, 1.5));
    }

    #[test]
    fn test_compression_levels() {
        let data = b"Test data for compression levels. ".repeat(100);

        // Fast compression
        let fast = compress(&data, Algorithm::Zstd, CompressionLevel::FAST).unwrap();
        
        // Default compression
        let default = compress(&data, Algorithm::Zstd, CompressionLevel::DEFAULT).unwrap();
        
        // Best compression
        let best = compress(&data, Algorithm::Zstd, CompressionLevel::BEST).unwrap();

        // Higher levels should compress better (smaller size)
        assert!(best.len() <= default.len());
        assert!(default.len() <= fast.len());

        // All should decompress correctly
        assert_eq!(data, decompress(&fast, Algorithm::Zstd).unwrap().as_slice());
        assert_eq!(data, decompress(&default, Algorithm::Zstd).unwrap().as_slice());
        assert_eq!(data, decompress(&best, Algorithm::Zstd).unwrap().as_slice());
    }

    #[test]
    fn test_lz4_vs_zstd() {
        let data = b"Comparing LZ4 and Zstandard compression algorithms. ".repeat(100);

        let lz4_compressed = compress(&data, Algorithm::Lz4, CompressionLevel::DEFAULT).unwrap();
        let zstd_compressed = compress(&data, Algorithm::Zstd, CompressionLevel::DEFAULT).unwrap();

        // Zstd typically has better compression ratio
        println!("LZ4: {} bytes", lz4_compressed.len());
        println!("Zstd: {} bytes", zstd_compressed.len());

        // Both should decompress correctly
        assert_eq!(data, decompress(&lz4_compressed, Algorithm::Lz4).unwrap().as_slice());
        assert_eq!(data, decompress(&zstd_compressed, Algorithm::Zstd).unwrap().as_slice());
    }

    /// Decompression-bomb defence: a tiny Zstd frame whose decompressed size
    /// vastly exceeds the supplied cap must be rejected, not allocated.
    #[test]
    fn test_zstd_decompression_bomb_rejected() {
        // 16 MiB of zeros compresses to a few bytes with Zstd.
        let original = vec![0u8; 16 * 1024 * 1024];
        let bomb = compress(&original, Algorithm::Zstd, CompressionLevel::DEFAULT).unwrap();
        assert!(bomb.len() < 1024, "test setup: bomb must be tiny");

        // With a 1 MiB cap, decompression must fail rather than balloon RAM.
        let err = decompress_with_limit(&bomb, Algorithm::Zstd, 1024 * 1024)
            .expect_err("bomb must be rejected");
        assert!(err.to_string().contains("exceeds limit"), "got: {err}");
    }

    #[test]
    fn test_lz4_decompression_bomb_rejected() {
        let original = vec![0u8; 16 * 1024 * 1024];
        let bomb = compress(&original, Algorithm::Lz4, CompressionLevel::DEFAULT).unwrap();
        assert!(bomb.len() < 1024 * 1024, "test setup: bomb must be far smaller than original");

        let err = decompress_with_limit(&bomb, Algorithm::Lz4, 1024 * 1024)
            .expect_err("bomb must be rejected");
        assert!(err.to_string().contains("exceeds limit"), "got: {err}");
    }

    #[test]
    fn test_within_limit_succeeds() {
        let original = b"hello world".repeat(100);
        let compressed = compress(&original, Algorithm::Zstd, CompressionLevel::DEFAULT).unwrap();
        let out = decompress_with_limit(&compressed, Algorithm::Zstd, 1024 * 1024).unwrap();
        assert_eq!(out, original);
    }
}

// Made with Bob
