# QLTP Network Protocol Specification v1.0

## Overview

The Quantum Leap Transfer Protocol (QLTP) Network Layer enables high-speed file transfers over standard networks by combining intelligent compression, deduplication, and optimized network protocols.

## Design Goals

1. **High Throughput**: Maximize effective data transfer rate
2. **Reliability**: Ensure data integrity and delivery
3. **Resumability**: Support interrupted transfer recovery
4. **Security**: Encrypt data in transit
5. **Efficiency**: Minimize protocol overhead
6. **Scalability**: Support parallel streams

## Protocol Stack

```
┌─────────────────────────────────────┐
│     Application Layer (CLI)         │
├─────────────────────────────────────┤
│   QLTP Transfer Protocol (Layer 7)  │
├─────────────────────────────────────┤
│   QLTP Network Layer (This Spec)    │
├─────────────────────────────────────┤
│   TLS 1.3 (Optional Encryption)     │
├─────────────────────────────────────┤
│   TCP / QUIC (Transport)            │
├─────────────────────────────────────┤
│   IP (Network)                      │
└─────────────────────────────────────┘
```

## Connection Establishment

### 1. Handshake Protocol

**Client → Server: HELLO**
```
┌────────────────────────────────────┐
│ Magic: 0x514C5450 (QLTP)          │ 4 bytes
│ Version: 0x01                      │ 1 byte
│ Flags: 0x00                        │ 1 byte
│ Client ID: UUID                    │ 16 bytes
│ Capabilities: Bitfield             │ 4 bytes
│ Timestamp: Unix epoch              │ 8 bytes
│ Reserved: 0x00                     │ 6 bytes
└────────────────────────────────────┘
Total: 40 bytes
```

**Server → Client: WELCOME**
```
┌────────────────────────────────────┐
│ Magic: 0x514C5450 (QLTP)          │ 4 bytes
│ Version: 0x01                      │ 1 byte
│ Status: 0x00 (OK) / Error code     │ 1 byte
│ Server ID: UUID                    │ 16 bytes
│ Session ID: UUID                   │ 16 bytes
│ Max Chunk Size: uint32             │ 4 bytes
│ Max Parallel Streams: uint16       │ 2 bytes
│ Timestamp: Unix epoch              │ 8 bytes
│ Reserved: 0x00                     │ 8 bytes
└────────────────────────────────────┘
Total: 60 bytes
```

### 2. Capability Negotiation

**Capabilities Bitfield** (32 bits):
```
Bit 0: Compression (LZ4)
Bit 1: Compression (Zstd)
Bit 2: Deduplication
Bit 3: Resume support
Bit 4: TLS encryption
Bit 5: QUIC support
Bit 6: Parallel streams
Bit 7: Delta encoding
Bit 8-15: Reserved
Bit 16-31: Custom extensions
```

## Message Format

### Base Message Header

All messages follow this format:

```
┌────────────────────────────────────┐
│ Message Type: uint8                │ 1 byte
│ Flags: uint8                       │ 1 byte
│ Sequence Number: uint32            │ 4 bytes
│ Session ID: UUID                   │ 16 bytes
│ Payload Length: uint32             │ 4 bytes
│ Checksum: CRC32                    │ 4 bytes
│ Payload: variable                  │ N bytes
└────────────────────────────────────┘
Total: 30 + N bytes
```

### Message Types

```
0x01: HELLO          - Connection initiation
0x02: WELCOME        - Connection acceptance
0x03: TRANSFER_START - Begin file transfer
0x04: CHUNK_DATA     - File chunk data
0x05: CHUNK_ACK      - Chunk acknowledgment
0x06: TRANSFER_END   - Complete transfer
0x07: ERROR          - Error notification
0x08: PING           - Keep-alive
0x09: PONG           - Keep-alive response
0x0A: RESUME_REQUEST - Resume interrupted transfer
0x0B: RESUME_ACK     - Resume acknowledgment
0x0C: METADATA       - File metadata
0x0D: GOODBYE        - Connection termination
```

## Transfer Protocol

### 1. Transfer Initiation

**Client → Server: TRANSFER_START**
```
┌────────────────────────────────────┐
│ Header (30 bytes)                  │
├────────────────────────────────────┤
│ Transfer ID: UUID                  │ 16 bytes
│ File Name Length: uint16           │ 2 bytes
│ File Name: UTF-8 string            │ N bytes
│ File Size: uint64                  │ 8 bytes
│ Total Chunks: uint32               │ 4 bytes
│ Chunk Size: uint32                 │ 4 bytes
│ Compression: uint8                 │ 1 byte
│ Hash Algorithm: uint8              │ 1 byte
│ File Hash: SHA-256                 │ 32 bytes
│ Metadata Length: uint16            │ 2 bytes
│ Metadata: JSON                     │ M bytes
└────────────────────────────────────┘
```

**Server → Client: TRANSFER_ACK**
```
┌────────────────────────────────────┐
│ Header (30 bytes)                  │
├────────────────────────────────────┤
│ Transfer ID: UUID                  │ 16 bytes
│ Status: uint8                      │ 1 byte
│ Resume Offset: uint64              │ 8 bytes
│ Available Space: uint64            │ 8 bytes
│ Preferred Chunk Size: uint32       │ 4 bytes
└────────────────────────────────────┘
```

### 2. Chunk Transfer

**Client → Server: CHUNK_DATA**
```
┌────────────────────────────────────┐
│ Header (30 bytes)                  │
├────────────────────────────────────┤
│ Transfer ID: UUID                  │ 16 bytes
│ Chunk Index: uint32                │ 4 bytes
│ Chunk Offset: uint64               │ 8 bytes
│ Original Size: uint32              │ 4 bytes
│ Compressed Size: uint32            │ 4 bytes
│ Chunk Hash: SHA-256                │ 32 bytes
│ Compression Type: uint8            │ 1 byte
│ Flags: uint8                       │ 1 byte
│ Chunk Data: bytes                  │ N bytes
└────────────────────────────────────┘
```

**Chunk Flags**:
```
Bit 0: Compressed
Bit 1: Deduplicated (reference only)
Bit 2: Last chunk
Bit 3: Encrypted
Bit 4-7: Reserved
```

**Server → Client: CHUNK_ACK**
```
┌────────────────────────────────────┐
│ Header (30 bytes)                  │
├────────────────────────────────────┤
│ Transfer ID: UUID                  │ 16 bytes
│ Chunk Index: uint32                │ 4 bytes
│ Status: uint8                      │ 1 byte
│ Received Size: uint32              │ 4 bytes
│ Timestamp: uint64                  │ 8 bytes
└────────────────────────────────────┘
```

### 3. Transfer Completion

**Client → Server: TRANSFER_END**
```
┌────────────────────────────────────┐
│ Header (30 bytes)                  │
├────────────────────────────────────┤
│ Transfer ID: UUID                  │ 16 bytes
│ Total Bytes Sent: uint64           │ 8 bytes
│ Total Chunks: uint32               │ 4 bytes
│ Compression Ratio: float32         │ 4 bytes
│ Transfer Duration: uint64 (ms)     │ 8 bytes
│ File Hash: SHA-256                 │ 32 bytes
└────────────────────────────────────┘
```

**Server → Client: TRANSFER_COMPLETE**
```
┌────────────────────────────────────┐
│ Header (30 bytes)                  │
├────────────────────────────────────┤
│ Transfer ID: UUID                  │ 16 bytes
│ Status: uint8                      │ 1 byte
│ Total Bytes Received: uint64       │ 8 bytes
│ Verified Hash: SHA-256             │ 32 bytes
│ Storage Path Length: uint16        │ 2 bytes
│ Storage Path: UTF-8 string         │ N bytes
└────────────────────────────────────┘
```

## Flow Control

### 1. Window-Based Flow Control

- **Send Window**: Maximum unacknowledged chunks (default: 256)
- **Receive Window**: Maximum buffered chunks (default: 512)
- **Dynamic Adjustment**: Based on RTT and packet loss

### 2. Congestion Control

Uses TCP-like congestion control:

```
Initial Window: 10 chunks
Slow Start Threshold: 64 chunks
Congestion Avoidance: Additive increase
Fast Retransmit: After 3 duplicate ACKs
Fast Recovery: Multiplicative decrease
```

### 3. Bandwidth Throttling

```
Target Rate = User Specified (bytes/sec)
Token Bucket Algorithm:
  - Bucket Size: 2 × Target Rate
  - Refill Rate: Target Rate
  - Consume: Chunk Size per send
```

## Error Handling

### Error Codes

```
0x00: SUCCESS
0x01: PROTOCOL_ERROR
0x02: INVALID_MESSAGE
0x03: UNSUPPORTED_VERSION
0x04: AUTHENTICATION_FAILED
0x05: INSUFFICIENT_SPACE
0x06: FILE_NOT_FOUND
0x07: PERMISSION_DENIED
0x08: CHECKSUM_MISMATCH
0x09: TIMEOUT
0x0A: CONNECTION_LOST
0x0B: TRANSFER_ABORTED
0x0C: RESOURCE_EXHAUSTED
```

### Retransmission Strategy

```
Initial Timeout: 1 second
Max Timeout: 60 seconds
Backoff: Exponential (2^n)
Max Retries: 5
```

## Resume Protocol

### 1. Resume Request

**Client → Server: RESUME_REQUEST**
```
┌────────────────────────────────────┐
│ Header (30 bytes)                  │
├────────────────────────────────────┤
│ Transfer ID: UUID                  │ 16 bytes
│ Last Chunk Index: uint32           │ 4 bytes
│ Last Chunk Offset: uint64          │ 8 bytes
│ Partial File Hash: SHA-256         │ 32 bytes
└────────────────────────────────────┘
```

**Server → Client: RESUME_ACK**
```
┌────────────────────────────────────┐
│ Header (30 bytes)                  │
├────────────────────────────────────┤
│ Transfer ID: UUID                  │ 16 bytes
│ Resume From Chunk: uint32          │ 4 bytes
│ Resume From Offset: uint64         │ 8 bytes
│ Status: uint8                      │ 1 byte
└────────────────────────────────────┘
```

## Security

### 1. TLS Encryption

- **Version**: TLS 1.3
- **Cipher Suites**: 
  - TLS_AES_256_GCM_SHA384
  - TLS_CHACHA20_POLY1305_SHA256
- **Certificate Validation**: Required for production

### 2. Authentication

**Token-Based Authentication**:
```
Authorization: Bearer <JWT_TOKEN>
```

**JWT Claims**:
```json
{
  "sub": "client_id",
  "iat": 1234567890,
  "exp": 1234571490,
  "scope": "transfer:read transfer:write"
}
```

### 3. Integrity Verification

- **Per-Chunk**: SHA-256 hash
- **Per-Message**: CRC32 checksum
- **End-to-End**: Full file SHA-256 hash

## Performance Optimizations

### 1. Parallel Streams

```
Max Streams: Negotiated (default: 4)
Stream Assignment: Round-robin by chunk
Stream Priority: Equal (can be adjusted)
```

### 2. Zero-Copy Transfer

```
sendfile() on Linux
TransmitFile() on Windows
Direct buffer mapping where possible
```

### 3. Adaptive Compression

```
Network Speed > 100 Mbps: LZ4 (fast)
Network Speed < 100 Mbps: Zstd (better ratio)
High Latency: Aggressive compression
Low Latency: Minimal compression
```

### 4. Predictive Pre-fetching

```
Analyze transfer patterns
Pre-fetch next chunks
Warm up compression pipeline
Pre-allocate buffers
```

## Implementation Notes

### 1. Buffer Management

```
Send Buffer: 16 MB (configurable)
Receive Buffer: 32 MB (configurable)
Chunk Buffer Pool: 256 chunks
Zero-copy where possible
```

### 2. Threading Model

```
Main Thread: Connection management
Send Thread: Chunk transmission
Receive Thread: ACK processing
Worker Pool: Compression/decompression
```

### 3. Memory Management

```
Pre-allocated buffers
Object pooling for chunks
Reference counting for shared data
Periodic garbage collection
```

## Protocol Extensions

### 1. Multicast Support (Future)

```
Message Type: 0x20-0x2F
One-to-many transfers
Reliable multicast protocol
```

### 2. P2P Support (Future)

```
Message Type: 0x30-0x3F
Peer discovery
Direct peer connections
NAT traversal
```

### 3. Cloud Integration (Future)

```
Message Type: 0x40-0x4F
S3/Azure/GCS support
Direct cloud uploads
Hybrid transfers
```

## Compatibility

### Version Negotiation

```
Client sends: Supported versions [1, 2, 3]
Server responds: Selected version [2]
Fallback: Lowest common version
```

### Backward Compatibility

```
Version 1.0: Base protocol (this spec)
Version 1.1: Add QUIC support
Version 2.0: Add P2P support
```

## Testing Requirements

### 1. Unit Tests

- Message serialization/deserialization
- Protocol state machine
- Error handling
- Flow control logic

### 2. Integration Tests

- End-to-end transfers
- Resume functionality
- Error recovery
- Performance benchmarks

### 3. Stress Tests

- High concurrency (1000+ connections)
- Large files (100+ GB)
- Network failures
- Resource exhaustion

## References

- RFC 793: TCP
- RFC 9000: QUIC
- RFC 8446: TLS 1.3
- RFC 7519: JWT

## Changelog

- **v1.0** (2026-04-13): Initial specification

---

**Document Status**: Draft  
**Last Updated**: 2026-04-13  
**Authors**: QLTP Development Team