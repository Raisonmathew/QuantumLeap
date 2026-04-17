# QLTP Licensing System

Complete licensing and access control system for QLTP following Domain-Driven Design (DDD) and Hexagonal Architecture principles.

## Overview

The licensing system provides:
- **5 License Tiers**: Free, Pro, Team, Business, Enterprise
- **Feature-based Access Control**: 14 features with tier-based restrictions
- **Device Management**: Track and limit activated devices per license
- **Usage Tracking**: Monitor data transfer quotas and enforce limits
- **Secure License Keys**: SHA256-based key generation and validation

## Architecture

### Layers

```
┌─────────────────────────────────────────┐
│         Application Layer               │
│  (LicenseService, UsageTracker)        │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│            Ports Layer                  │
│  (LicenseRepository, UsageRepository)   │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│          Adapters Layer                 │
│  (MemoryStore, SQLiteStore)            │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│          Domain Layer                   │
│  (License, Device, Quota, etc.)        │
└─────────────────────────────────────────┘
```

## License Tiers

| Tier | Monthly Quota | Max File Size | Devices | Price | Features |
|------|--------------|---------------|---------|-------|----------|
| **Free** | 10 GB | 100 MB | 1 | $0 | Basic transfers, compression |
| **Pro** | 100 GB | 1 GB | 3 | $9.99 | + Encryption, deduplication |
| **Team** | 500 GB | 5 GB | 10 | $49.99 | + Parallel transfers, QUIC |
| **Business** | 2 TB | 50 GB | 50 | $199 | + Priority support, analytics |
| **Enterprise** | Unlimited | Unlimited | Unlimited | Custom | + SSO, API access, SLA |

## Features

1. **Compression** - Data compression during transfer
2. **Encryption** - End-to-end encryption
3. **Deduplication** - Block-level deduplication
4. **ParallelTransfers** - Multiple concurrent transfers
5. **QUIC** - QUIC protocol support
6. **Resumable** - Resume interrupted transfers
7. **Scheduling** - Scheduled transfers
8. **PrioritySupport** - Priority customer support
9. **Analytics** - Usage analytics and reporting
10. **Collaboration** - Team collaboration features
11. **APIAccess** - Programmatic API access
12. **SSO** - Single Sign-On integration
13. **CustomBranding** - White-label branding
14. **SLA** - Service Level Agreement

## Usage Examples

### Creating a License

```rust
use qltp_licensing::{LicenseService, LicenseTier, MemoryLicenseStore};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let repo = Arc::new(MemoryLicenseStore::new());
    let service = LicenseService::new(repo);
    
    // Create a Pro license
    let license = service.create_license(
        LicenseTier::Pro,
        Some("user@example.com".to_string())
    ).await.unwrap();
    
    println!("License Key: {}", license.key());
}
```

### Activating a Device

```rust
use qltp_licensing::DeviceFingerprint;

// Generate device fingerprint
let fingerprint = DeviceFingerprint::generate();

// Activate device
service.activate_device(
    &license_key,
    "My Laptop".to_string(),
    fingerprint
).await.unwrap();
```

### Checking Feature Access

```rust
use qltp_licensing::Feature;

// Check if encryption is available
let result = service.validate_license_for_feature(
    &license_key,
    Feature::Encryption
).await;

if result.is_ok() {
    println!("Encryption is available!");
}
```

### Tracking Usage

```rust
use qltp_licensing::{UsageTracker, TransferType, LicenseId};

let usage_tracker = UsageTracker::new(license_repo, usage_repo);

// Check quota before transfer
let license_id = LicenseId::new();
let file_size = 1024 * 1024 * 100; // 100 MB

usage_tracker.check_quota(&license_id, file_size).await?;

// Record transfer
usage_tracker.record_transfer(
    license_id,
    file_size,
    TransferType::Upload
).await?;

// Get usage statistics
let stats = usage_tracker.get_usage_stats(
    &license_id,
    start_date,
    end_date
).await?;

println!("Total: {}", stats.total_size_human());
println!("Uploads: {}", stats.upload_size_human());
println!("Downloads: {}", stats.download_size_human());
```

### Upgrading License Tier

```rust
// Upgrade from Pro to Team
let upgraded = service.upgrade_tier(
    &license_key,
    LicenseTier::Team
).await.unwrap();

println!("Upgraded to: {}", upgraded.tier());
```

## License Key Format

License keys follow the format:
```
QLTP-{TIER}-{SEGMENT1}-{SEGMENT2}-{CHECKSUM}
```

Example:
```
QLTP-PRO-A1B2-C3D4-E5F6
```

- **TIER**: License tier (FREE, PRO, TEAM, BUSINESS, ENTERPRISE)
- **SEGMENT1/2**: Random alphanumeric segments
- **CHECKSUM**: SHA256-based validation checksum

## Device Fingerprinting

Device fingerprints are generated from:
- Hostname
- Operating System
- Architecture
- Username

This creates a unique identifier for each device while maintaining privacy.

## Quota Management

### Monthly Limits

Quotas are enforced on a rolling 30-day basis:

```rust
// Get remaining quota
let remaining = usage_tracker.get_remaining_quota(&license_id).await?;

if let Some(bytes) = remaining {
    println!("Remaining: {} bytes", bytes);
} else {
    println!("Unlimited quota");
}
```

### File Size Limits

Each tier has maximum file size limits enforced before transfer:

```rust
let quota = Quota::for_tier(LicenseTier::Pro);
if !quota.is_file_size_allowed(file_size) {
    return Err("File too large for this tier");
}
```

## Error Handling

The system provides comprehensive error types:

```rust
use qltp_licensing::LicenseError;

match result {
    Err(LicenseError::LicenseExpired) => {
        println!("License has expired");
    }
    Err(LicenseError::QuotaExceeded { message }) => {
        println!("Quota exceeded: {}", message);
    }
    Err(LicenseError::FeatureNotAvailable { tier }) => {
        println!("Feature not available in {} tier", tier);
    }
    Err(LicenseError::DeviceLimitExceeded { max }) => {
        println!("Maximum {} devices allowed", max);
    }
    Ok(_) => println!("Success!"),
}
```

## Testing

The system includes 72 comprehensive tests:

```bash
# Run all licensing tests
cargo test --package qltp-licensing

# Run specific test suite
cargo test --package qltp-licensing domain::license
cargo test --package qltp-licensing adapters
cargo test --package qltp-licensing application
```

## Integration with QLTP

### Transfer Validation

Before initiating a transfer:

1. Validate license is active and not expired
2. Check feature access (e.g., encryption, QUIC)
3. Verify quota limits
4. Record usage after successful transfer

```rust
// Validate before transfer
service.validate_license_for_feature(&key, Feature::Encryption).await?;
usage_tracker.check_quota(&license_id, file_size).await?;

// Perform transfer
transfer_file().await?;

// Record usage
usage_tracker.record_transfer(license_id, bytes_transferred, TransferType::Upload).await?;
```

## Storage Adapters

### In-Memory (Development/Testing)

```rust
use qltp_licensing::{MemoryLicenseStore, MemoryUsageStore};

let license_repo = Arc::new(MemoryLicenseStore::new());
let usage_repo = Arc::new(MemoryUsageStore::new());
```

### SQLite (Production - Coming Soon)

```rust
use qltp_licensing::{SQLiteLicenseStore, SQLiteUsageStore};

let license_repo = Arc::new(SQLiteLicenseStore::new("licenses.db")?);
let usage_repo = Arc::new(SQLiteUsageStore::new("usage.db")?);
```

## CLI Commands (Coming Soon)

```bash
# Activate license
qltp license activate QLTP-PRO-XXXX-YYYY-ZZZZ

# Check license status
qltp license status

# List devices
qltp license devices

# Deactivate device
qltp license deactivate <device-id>

# View usage
qltp license usage

# Upgrade tier
qltp license upgrade --tier team
```

## API Reference

### LicenseService

- `create_license(tier, email)` - Create new license
- `activate_license(key)` - Activate license with key
- `activate_device(key, name, fingerprint)` - Activate device
- `deactivate_device(key, device_id)` - Deactivate device
- `upgrade_tier(key, new_tier)` - Upgrade license tier
- `link_user(key, token)` - Link license to user
- `get_license(key)` - Get license details
- `validate_license_for_feature(key, feature)` - Check feature access
- `get_quota(key)` - Get quota information

### UsageTracker

- `record_transfer(license_id, bytes, type)` - Record transfer
- `check_quota(license_id, bytes)` - Check quota limits
- `get_usage_stats(license_id, start, end)` - Get usage statistics
- `get_current_month_usage(license_id)` - Get current month usage
- `get_remaining_quota(license_id)` - Get remaining quota
- `cleanup_old_records(before)` - Clean up old records

## Security Considerations

1. **License Key Validation**: SHA256 checksums prevent tampering
2. **Device Fingerprinting**: Unique device identification
3. **Quota Enforcement**: Server-side validation prevents bypass
4. **Feature Flags**: Tier-based access control
5. **Expiration Checks**: Automatic license expiration handling

## Performance

- **In-Memory Storage**: O(1) lookups, suitable for development
- **Async Operations**: Non-blocking I/O for all repository operations
- **Efficient Queries**: Indexed lookups by license key and user ID
- **Batch Operations**: Support for bulk license operations

## Future Enhancements

1. **SQLite Adapter**: Persistent storage implementation
2. **License Server**: Centralized license validation service
3. **Offline Mode**: Grace period for offline validation
4. **License Transfer**: Transfer licenses between users
5. **Usage Analytics**: Detailed usage reports and dashboards
6. **Webhook Integration**: Real-time license event notifications

## Support

For issues or questions:
- GitHub Issues: [qltp-project/issues](https://github.com/qltp-project/issues)
- Documentation: [docs.qltp.io](https://docs.qltp.io)
- Email: support@qltp.io