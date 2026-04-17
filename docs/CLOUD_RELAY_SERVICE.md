# Cloud Relay Service - Backend Server Design

## Overview

The Cloud Relay Service is a lightweight backend server that coordinates P2P file transfers between QLTP clients. It handles metadata, peer discovery, NAT traversal, and session management without touching the actual file data.

## Architecture Principles

### Key Design Goals
1. **Metadata Only** - Server never handles file data, only coordination
2. **Scalable** - Support thousands of concurrent transfers
3. **Lightweight** - Minimal resource usage per transfer
4. **Secure** - End-to-end encryption, server only sees metadata
5. **Fast** - Low latency for coordination operations

### Data Flow

```
┌─────────────┐                                    ┌─────────────┐
│   Sender    │                                    │  Receiver   │
│   (Alice)   │                                    │    (Bob)    │
└──────┬──────┘                                    └──────┬──────┘
       │                                                  │
       │ 1. Create Transfer                               │
       ├──────────────────────────────────────────────────┤
       │                                                  │
       │         ┌─────────────────────────┐             │
       │         │   Cloud Relay Service   │             │
       │         │  (Metadata + Signaling) │             │
       │         └─────────────────────────┘             │
       │                     │                            │
       │ 2. Get Transfer ID  │                            │
       │◄────────────────────┘                            │
       │                                                  │
       │ 3. Share Transfer ID (out of band)               │
       ├─────────────────────────────────────────────────►│
       │                                                  │
       │                                    4. Join Transfer
       │                                                  ├──┐
       │                                                  │  │
       │         ┌─────────────────────────┐             │  │
       │         │   Get Peer Info         │◄────────────┘  │
       │         │   (IP, Port, NAT type)  │                │
       │         └─────────────────────────┘                │
       │                     │                              │
       │ 5. Exchange Peer Info                             │
       │◄────────────────────┴──────────────────────────────┤
       │                                                    │
       │ 6. Direct P2P Connection (data transfer)          │
       │◄──────────────────────────────────────────────────►│
       │          (Server not involved in data)            │
       │                                                    │
       │ 7. Report Progress                                │
       ├──────────────────────────────────────────────────►│
       │                                                    │
       │ 8. Complete Transfer                              │
       ├──────────────────────────────────────────────────►│
       │                                                    │
```

## API Design

### REST API Endpoints

#### 1. Create Transfer
```http
POST /api/v1/transfers
Content-Type: application/json
Authorization: Bearer <token>

{
  "file_name": "large-file.zip",
  "file_size": 1073741824,
  "file_hash": "sha256:abc123...",
  "sender_id": "alice@example.com",
  "transport_type": "io_uring",
  "encryption": "aes-256-gcm",
  "expires_in": 3600
}

Response 201:
{
  "transfer_id": "xfer_abc123",
  "access_code": "1234-5678",
  "expires_at": "2024-01-01T12:00:00Z",
  "sender_endpoint": {
    "ip": "203.0.113.1",
    "port": 8080,
    "nat_type": "symmetric"
  }
}
```

#### 2. Join Transfer
```http
POST /api/v1/transfers/{transfer_id}/join
Content-Type: application/json
Authorization: Bearer <token>

{
  "access_code": "1234-5678",
  "receiver_id": "bob@example.com"
}

Response 200:
{
  "transfer_id": "xfer_abc123",
  "file_name": "large-file.zip",
  "file_size": 1073741824,
  "file_hash": "sha256:abc123...",
  "sender_endpoint": {
    "ip": "203.0.113.1",
    "port": 8080,
    "nat_type": "symmetric"
  },
  "receiver_endpoint": {
    "ip": "198.51.100.1",
    "port": 9090,
    "nat_type": "cone"
  },
  "transport_type": "io_uring",
  "encryption": "aes-256-gcm"
}
```

#### 3. Update Transfer Status
```http
PATCH /api/v1/transfers/{transfer_id}/status
Content-Type: application/json
Authorization: Bearer <token>

{
  "status": "in_progress",
  "bytes_transferred": 536870912,
  "throughput_bps": 1000000000
}

Response 200:
{
  "transfer_id": "xfer_abc123",
  "status": "in_progress",
  "progress": 0.5
}
```

#### 4. Complete Transfer
```http
POST /api/v1/transfers/{transfer_id}/complete
Content-Type: application/json
Authorization: Bearer <token>

{
  "status": "completed",
  "bytes_transferred": 1073741824,
  "duration_seconds": 8.5,
  "average_throughput_bps": 1009175040
}

Response 200:
{
  "transfer_id": "xfer_abc123",
  "status": "completed",
  "stats": {
    "duration": 8.5,
    "throughput": "1.01 GB/s"
  }
}
```

#### 5. Get Transfer Info
```http
GET /api/v1/transfers/{transfer_id}
Authorization: Bearer <token>

Response 200:
{
  "transfer_id": "xfer_abc123",
  "status": "in_progress",
  "file_name": "large-file.zip",
  "file_size": 1073741824,
  "bytes_transferred": 536870912,
  "progress": 0.5,
  "created_at": "2024-01-01T11:00:00Z",
  "expires_at": "2024-01-01T12:00:00Z"
}
```

#### 6. List Transfers
```http
GET /api/v1/transfers?status=active&limit=10
Authorization: Bearer <token>

Response 200:
{
  "transfers": [
    {
      "transfer_id": "xfer_abc123",
      "file_name": "large-file.zip",
      "status": "in_progress",
      "progress": 0.5
    }
  ],
  "total": 1,
  "page": 1
}
```

### WebSocket API (Real-time Updates)

```javascript
// Connect to WebSocket
ws://relay.qltp.io/ws/transfers/{transfer_id}?token=<token>

// Server -> Client: Status updates
{
  "type": "status_update",
  "transfer_id": "xfer_abc123",
  "status": "in_progress",
  "bytes_transferred": 536870912,
  "throughput_bps": 1000000000
}

// Server -> Client: Peer connected
{
  "type": "peer_connected",
  "transfer_id": "xfer_abc123",
  "peer_id": "bob@example.com",
  "peer_endpoint": {
    "ip": "198.51.100.1",
    "port": 9090
  }
}

// Client -> Server: Heartbeat
{
  "type": "heartbeat",
  "transfer_id": "xfer_abc123"
}
```

## Data Models

### Transfer
```rust
struct Transfer {
    id: TransferId,
    file_name: String,
    file_size: u64,
    file_hash: String,
    sender_id: UserId,
    receiver_id: Option<UserId>,
    sender_endpoint: Endpoint,
    receiver_endpoint: Option<Endpoint>,
    transport_type: TransportType,
    encryption: EncryptionType,
    status: TransferStatus,
    bytes_transferred: u64,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

enum TransferStatus {
    Pending,      // Created, waiting for receiver
    Active,       // Both peers connected
    InProgress,   // Data transfer in progress
    Completed,    // Successfully completed
    Failed,       // Failed with error
    Expired,      // Expired before completion
    Cancelled,    // Cancelled by user
}

struct Endpoint {
    ip: IpAddr,
    port: u16,
    nat_type: NatType,
}

enum NatType {
    None,         // Public IP, no NAT
    FullCone,     // Easy to traverse
    RestrictedCone,
    PortRestrictedCone,
    Symmetric,    // Hardest to traverse
}
```

### User
```rust
struct User {
    id: UserId,
    email: String,
    license_key: String,
    license_tier: LicenseTier,
    created_at: DateTime<Utc>,
    last_seen: DateTime<Utc>,
}
```

## Technology Stack

### Backend Framework
- **Rust + Axum** - High-performance async web framework
- **Tower** - Middleware and service composition
- **Tokio** - Async runtime

### Database
- **PostgreSQL** - Primary data store
  - Transfers table
  - Users table
  - Sessions table
- **Redis** - Caching and real-time data
  - Active transfers cache
  - WebSocket connections
  - Rate limiting

### Infrastructure
- **Docker** - Containerization
- **Kubernetes** - Orchestration (optional)
- **NGINX** - Reverse proxy and load balancing
- **Let's Encrypt** - TLS certificates

## Implementation Plan

### Phase 1: Core API (Week 1)
1. **Project Setup**
   - Create new crate: `qltp-relay-server`
   - Set up Axum web server
   - Configure database connections
   - Set up logging and tracing

2. **Transfer Management**
   - Create transfer endpoint
   - Join transfer endpoint
   - Get transfer info endpoint
   - List transfers endpoint

3. **Database Schema**
   - Transfers table
   - Users table
   - Migrations

4. **Authentication**
   - JWT token validation
   - License key verification
   - Rate limiting

### Phase 2: Real-time Features (Week 2)
1. **WebSocket Support**
   - WebSocket connection handling
   - Real-time status updates
   - Peer discovery notifications

2. **NAT Traversal**
   - STUN server integration
   - NAT type detection
   - Hole punching coordination

3. **Session Management**
   - Active session tracking
   - Heartbeat monitoring
   - Automatic cleanup

### Phase 3: Production Features (Week 3)
1. **Monitoring & Metrics**
   - Prometheus metrics
   - Health check endpoints
   - Performance monitoring

2. **Security Hardening**
   - Rate limiting per user
   - DDoS protection
   - Input validation

3. **Deployment**
   - Docker image
   - Kubernetes manifests
   - CI/CD pipeline

## Security Considerations

### End-to-End Encryption
- Server never sees file data
- Encryption keys exchanged directly between peers
- Server only handles encrypted metadata

### Authentication
- JWT tokens for API access
- License key validation
- Per-user rate limiting

### Data Privacy
- Minimal data retention
- Automatic cleanup of expired transfers
- No logging of file contents

## Scalability

### Horizontal Scaling
- Stateless API servers
- Redis for shared state
- Database connection pooling

### Performance Targets
- **Latency**: < 50ms for API calls
- **Throughput**: 10,000 concurrent transfers
- **Availability**: 99.9% uptime

### Resource Usage
- **CPU**: < 10% per 1000 transfers
- **Memory**: < 100 MB per 1000 transfers
- **Database**: < 1 KB per transfer record

## Monitoring

### Key Metrics
- Active transfers count
- Transfer success rate
- Average transfer duration
- API response times
- WebSocket connection count
- Database query performance

### Alerts
- High error rate
- Database connection issues
- Memory/CPU threshold exceeded
- Unusual traffic patterns

## Cost Estimation

### Infrastructure (Monthly)
- **Server**: $50-100 (2-4 vCPUs, 4-8 GB RAM)
- **Database**: $25-50 (PostgreSQL managed service)
- **Redis**: $15-30 (Managed Redis)
- **Bandwidth**: $10-50 (metadata only, minimal)
- **Total**: ~$100-230/month for 10,000 transfers/day

## Future Enhancements

### Phase 4+
1. **Multi-region Support**
   - Deploy in multiple regions
   - Geo-routing for low latency

2. **Advanced NAT Traversal**
   - TURN server for difficult NATs
   - Relay fallback when P2P fails

3. **Analytics Dashboard**
   - Transfer statistics
   - User analytics
   - Performance insights

4. **Mobile Support**
   - iOS/Android SDKs
   - Push notifications

5. **Enterprise Features**
   - Team management
   - Admin dashboard
   - Audit logs
   - Custom branding

## References

- [WebRTC NAT Traversal](https://webrtc.org/getting-started/peer-connections)
- [STUN/TURN Protocols](https://tools.ietf.org/html/rfc5389)
- [Axum Web Framework](https://docs.rs/axum/)
- [PostgreSQL Best Practices](https://wiki.postgresql.org/wiki/Performance_Optimization)