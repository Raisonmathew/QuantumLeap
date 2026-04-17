# QLTP Quick Start Guide

Get up and running with QLTP in 5 minutes!

## Installation

### Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Build QLTP

```bash
# Clone or navigate to the project
cd qltp-project

# Build in release mode (optimized)
cargo build --release

# The binary will be at: target/release/qltp
```

## Quick Examples

### 1. Show System Information

```bash
cargo run --release --bin qltp -- info
```

Output:
```
🚀 QLTP - Quantum Leap Transfer Protocol
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Version: 0.1.0
...
```

### 2. Analyze a File

Create a test file and analyze it:

```bash
# Create a 10MB test file
dd if=/dev/zero of=test_10mb.bin bs=1M count=10

# Analyze with default 4KB chunks
cargo run --release --bin qltp -- analyze test_10mb.bin
```

Output:
```
🔍 QLTP File Analysis
File: test_10mb.bin

File size: 10485760 bytes (10.00 MB)
Chunk size: 4096 bytes
Chunking method: Fixed-size

📦 Chunking Results
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Total chunks: 2560
Average chunk size: 4096 bytes
...
```

### 3. Content-Defined Chunking

```bash
# Use content-defined chunking for better deduplication
cargo run --release --bin qltp -- analyze test_10mb.bin --content-defined
```

### 4. Transfer a File (Simulated)

```bash
# Create source file
echo "Hello, QLTP!" > source.txt

# Transfer (currently simulated)
cargo run --release --bin qltp -- transfer source.txt remote:/path/dest.txt
```

Output:
```
🚀 QLTP Transfer
Source: source.txt
Destination: remote:/path/dest.txt

File size: 13 bytes (0.00 MB)

✅ Transfer Complete
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Bytes transferred: 13 bytes
Duration: 0.00s
Speed: 0.01 MB/s
...
```

## Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_chunking_integration

# Run tests for specific crate
cargo test -p qltp-core
cargo test -p qltp-compression
cargo test -p qltp-storage
```

## Performance Testing

### Benchmark Chunking

```bash
# Create a large test file (100MB)
dd if=/dev/zero of=test_100mb.bin bs=1M count=100

# Analyze performance
time cargo run --release --bin qltp -- analyze test_100mb.bin
```

### Compare Chunking Methods

```bash
# Fixed-size chunking
time cargo run --release --bin qltp -- analyze test_100mb.bin

# Content-defined chunking
time cargo run --release --bin qltp -- analyze test_100mb.bin --content-defined
```

## Advanced Usage

### Custom Chunk Size

```bash
# Use 64KB chunks
cargo run --release --bin qltp -- analyze file.bin --chunk-size 65536
```

### Verbose Logging

```bash
# Enable verbose output
cargo run --release --bin qltp -- -v transfer source.txt dest.txt

# Enable debug output
cargo run --release --bin qltp -- -d transfer source.txt dest.txt
```

### Transfer Options

```bash
# Disable compression
cargo run --release --bin qltp -- transfer source.txt dest.txt --no-compression

# Disable deduplication
cargo run --release --bin qltp -- transfer source.txt dest.txt --no-dedup

# Enable encryption
cargo run --release --bin qltp -- transfer source.txt dest.txt --encrypt
```

## What's Working Now

✅ **Core Engine**
- File chunking (fixed-size and content-defined)
- SHA-256 and BLAKE3 hashing
- Error handling and logging

✅ **Compression Layer**
- LZ4 compression (500+ MB/s)
- Zstandard compression (adjustable levels)
- Automatic compression detection

✅ **Storage Layer**
- Content-addressable storage
- Deduplication engine
- Reference counting

✅ **CLI Tool**
- File analysis
- Transfer simulation
- Progress tracking
- Verbose logging

## What's Coming Next

🚧 **In Development**
- Real network transfer (TCP/QUIC)
- Progress bars for transfers
- Resume capability
- Neural compression (Layer 4)

📋 **Planned**
- Desktop application
- Mobile applications
- Cloud service
- Enterprise middleware

## Troubleshooting

### Build Fails

```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean
cargo build --release
```

### Tests Fail

```bash
# Run tests with backtrace
RUST_BACKTRACE=1 cargo test

# Run single test
cargo test test_name -- --nocapture
```

### Performance Issues

```bash
# Ensure you're using release mode
cargo build --release

# Check system resources
top
df -h
```

## Next Steps

1. **Read the full documentation**: [GETTING_STARTED.md](GETTING_STARTED.md)
2. **Review the business plan**: [BUSINESS_PLAN.md](BUSINESS_PLAN.md)
3. **Explore the code**: Start with `crates/qltp-core/src/lib.rs`
4. **Run the tests**: `cargo test`
5. **Contribute**: See [CONTRIBUTING.md](../CONTRIBUTING.md) (TODO)

## Support

- **Documentation**: [docs/](.)
- **Issues**: GitHub Issues (TODO)
- **Email**: hello@qltp.io
- **Website**: https://qltp.io

## License

Dual-licensed under MIT/Apache 2.0