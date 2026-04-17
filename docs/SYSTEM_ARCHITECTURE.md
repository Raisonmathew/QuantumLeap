# QLTP Complete System Architecture

## Executive Summary

QLTP (Quantum Leap Transfer Protocol) is a high-performance P2P file transfer system with cloud-based authentication, licensing, and usage tracking. The system consists of three main components:

1. **CLI Client** - P2P file transfer application (runs on user devices)
2. **Backend Server** - Cloud API for auth, licensing, payments (runs on cloud)
3. **Shared Libraries** - Core transfer logic used by both

## System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         QLTP ECOSYSTEM                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐                    ┌──────────────────┐      │
│  │   User Device A  │                    │   User Device B  │      │
│  │  ┌────────────┐  │                    │  ┌────────────┐  │      │
│  │  │ QLTP CLI   │  │◄──────P2P─────────►│  │ QLTP CLI   │  │      │
│  │  │  (Sender)  │  │   File Transfer    │  │ (Receiver) │  │      │
│  │  └─────┬──────┘  │                    │  └─────┬──────┘  │      │
│  └────────┼─────────┘                    └────────┼─────────┘      │
│           │                                       │                 │
│           │  Auth/License                         │  Auth/License  │
│           │  Validation                           │  Validation    │
│           │                                       │                 │
│           └───────────────┐       ┌───────────────┘                 │
│                           │       │                                 │
│                           ▼       ▼                                 │
│                    ┌──────────────────┐                             │
│                    │  Backend Server  │                             │
│                    │   (Cloud API)    │                             │
│                    ├──────────────────┤                             │
│                    │ • Authentication │                             │
│                    │ • License Mgmt   │                             │
│                    │ • Usage Tracking │                             │
│                    │ • Payments       │                             │
│                    │ • User Accounts  │                             │
│                    └────────┬─────────┘                             │
│                             │                                       │
│                             ▼                                       │
│                    ┌──────────────────┐                             │
│                    │   PostgreSQL DB  │                             │
│                    │  • Users         │                             │
│                    │  • Licenses      │                             │
│                    │  • Devices       │                             │
│                    │  • Usage Records │                             │
│                    └──────────────────┘                             │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Key Insight: P2P Transfer + Cloud Management

**QLTP is NOT a cloud file transfer service.** It's a P2P file transfer protocol with cloud-based management:

- **File Transfer**: Direct P2P between devices (no files go through backend)
- **Backend Server**: Only handles auth, licensing, and usage tracking
- **Hybrid Model**: Best of both worlds - P2P speed + cloud management

## Component Breakdown

### 1. CLI Client (P2P Transfer Application)

**Location**: `apps/cli/`

**Purpose**: Runs on user devices to send/receive files directly between peers

**Key Features**:
- P2P file transfer (sender connects directly to receiver)
- Chunking, compression, deduplication
- Resume capability
- TLS encryption
- Local license validation
- Usage reporting to backend

**Commands**:
```bash
# P2P Transfer (no backend involved in file data)
qltp send file.bin 192.168.1.100:8080
qltp receive -l 0.0.0.0:8080 -o ./downloads

# Account Management (talks to backend)
qltp auth register user@example.com
qltp auth login user@example.com
qltp license activate PRO-XXXX-XXXX-XXXX
qltp license status
qltp usage show
```

**Architecture**:
```
CLI Application
├── Commands
│   ├── send/receive     → P2P file transfer
│   ├── auth             → Backend API calls
│   ├── license          → Backend API calls
│   └── usage            → Backend API calls
├── Uses Libraries
│   ├── qltp-core        → Chunking, hashing
│   ├── qltp-compression → LZ4, Zstd
│   ├── qltp-storage     → Content-addressable storage
│   ├── qltp-network     → TCP/QUIC, TLS, protocol
│   ├── qltp-auth        → Local session management
│   └── qltp-licensing   → License validation, quota checks
└── Talks To
    ├── Peer Device      → Direct TCP/QUIC connection
    └── Backend Server   → HTTPS REST API
```

### 2. Backend Server (Cloud API)

**Location**: `apps/backend-server/` (to be created in Phase 4)

**Purpose**: Cloud service for authentication, licensing, payments, and usage tracking

**Key Features**:
- User registration and authentication
- License activation and validation
- Device management
- Usage tracking and analytics
- Stripe payment integration
- Multi-device sync

**Does NOT**:
- ❌ Handle file transfers (files go P2P)
- ❌ Store user files
- ❌ Act as relay/proxy for transfers

**Architecture**:
```
Backend Server (Axum)
├── API Endpoints
│   ├── /api/v1/auth/*        → Authentication
│   ├── /api/v1/licenses/*    → License management
│   ├── /api/v1/usage/*       → Usage tracking
│   ├── /api/v1/payments/*    → Stripe integration
│   └── /api/v1/users/*       → User management
├── Services
│   ├── AuthService           → JWT, sessions
│   ├── LicenseService        → License logic
│   ├── PaymentService        → Stripe integration
│   └── EmailService          → Notifications
├── Database (PostgreSQL)
│   ├── users                 → User accounts
│   ├── licenses              → License keys
│   ├── license_devices       → Device activations
│   └── usage_records         → Transfer usage
└── Uses Libraries
    ├── qltp-auth             → Auth logic
    └── qltp-licensing        → License logic
```

### 3. Shared Libraries

**Location**: `crates/`

**Purpose**: Core functionality shared between CLI and Backend

**Crates**:

| Crate | Used By | Purpose |
|-------|---------|---------|
| `qltp-core` | CLI | Chunking, hashing, pipeline |
| `qltp-compression` | CLI | LZ4/Zstd compression |
| `qltp-storage` | CLI | Content-addressable storage |
| `qltp-network` | CLI | TCP/QUIC protocol, TLS |
| `qltp-auth` | CLI + Backend | Authentication logic |
| `qltp-licensing` | CLI + Backend | License validation, quotas |

## Data Flow Examples

### Example 1: User Purchases and Activates License

```
1. User visits website → Clicks "Buy Pro"
2. Website → POST /api/v1/payments/checkout (Backend)
3. Backend creates Stripe checkout session
4. User completes payment on Stripe
5. Stripe → POST /api/v1/payments/webhook (Backend)
6. Backend:
   - Generates license key: PRO-XXXX-XXXX-XXXX
   - Stores in database
   - Sends email with key
7. User runs: qltp license activate PRO-XXXX-XXXX-XXXX
8. CLI → POST /api/v1/licenses/activate (Backend)
9. Backend validates and activates license
10. CLI stores license locally
11. User can now use Pro features
```

### Example 2: P2P File Transfer with License Validation

```
Device A (Sender):
1. User runs: qltp send file.bin 192.168.1.100:8080
2. CLI checks local license (cached from backend)
3. CLI validates quota locally
4. CLI connects directly to Device B (P2P)
5. CLI sends file chunks over TCP/QUIC
6. After transfer, CLI → POST /api/v1/usage/report (Backend)

Device B (Receiver):
1. User runs: qltp receive -l 0.0.0.0:8080
2. CLI checks local license
3. CLI listens on port 8080
4. Device A connects (P2P)
5. CLI receives file chunks
6. After transfer, CLI → POST /api/v1/usage/report (Backend)

Backend:
- Receives usage reports from both devices
- Updates quota usage in database
- Sends email if quota nearly exhausted
```

### Example 3: Multi-Device Sync

```
1. User on Device A: qltp auth login alice@example.com
2. CLI → POST /api/v1/auth/login (Backend)
3. Backend:
   - Validates credentials
   - Returns JWT token + license info
   - Returns: { tier: "Pro", devices: 2/3, quota: 50GB/100GB }
4. CLI stores session locally
5. User switches to Device B
6. User on Device B: qltp auth login alice@example.com
7. Same flow, gets same license info
8. Both devices now have Pro features
```

## Backend Server API Design

### Authentication Endpoints

```
POST   /api/v1/auth/register
Request:  { email, password }
Response: { user_id, email, created_at }

POST   /api/v1/auth/login
Request:  { email, password }
Response: { token, user, license }

POST   /api/v1/auth/logout
Headers:  Authorization: Bearer <token>
Response: { success: true }

POST   /api/v1/auth/refresh
Headers:  Authorization: Bearer <token>
Response: { token }
```

### License Endpoints

```
POST   /api/v1/licenses/activate
Headers:  Authorization: Bearer <token>
Request:  { license_key, device_id, device_name }
Response: { license, device, quota }

GET    /api/v1/licenses/validate
Headers:  Authorization: Bearer <token>
Query:    ?license_key=PRO-XXXX
Response: { valid: true, tier, features, quota }

GET    /api/v1/licenses/current
Headers:  Authorization: Bearer <token>
Response: { license, devices, quota, usage }

POST   /api/v1/licenses/upgrade
Headers:  Authorization: Bearer <token>
Request:  { current_key, new_tier }
Response: { new_license, payment_url }

GET    /api/v1/licenses/devices
Headers:  Authorization: Bearer <token>
Response: { devices: [{ id, name, activated_at, last_seen }] }

DELETE /api/v1/licenses/devices/:device_id
Headers:  Authorization: Bearer <token>
Response: { success: true }
```

### Usage Endpoints

```
POST   /api/v1/usage/report
Headers:  Authorization: Bearer <token>
Request:  { license_key, bytes_transferred, transfer_type, timestamp }
Response: { quota_used, quota_remaining, quota_exceeded }

GET    /api/v1/usage/stats
Headers:  Authorization: Bearer <token>
Query:    ?period=month
Response: { total_bytes, transfers_count, quota_used, quota_limit }

GET    /api/v1/usage/history
Headers:  Authorization: Bearer <token>
Query:    ?start_date=2024-01-01&end_date=2024-01-31
Response: { records: [{ date, bytes, transfers }] }
```

### Payment Endpoints

```
POST   /api/v1/payments/checkout
Headers:  Authorization: Bearer <token>
Request:  { tier, billing_period }
Response: { checkout_url, session_id }

POST   /api/v1/payments/webhook
Headers:  Stripe-Signature: <signature>
Request:  <Stripe webhook payload>
Response: { received: true }

GET    /api/v1/payments/invoices
Headers:  Authorization: Bearer <token>
Response: { invoices: [{ id, date, amount, status, pdf_url }] }
```

### User Endpoints

```
GET    /api/v1/users/profile
Headers:  Authorization: Bearer <token>
Response: { user, license, devices, usage }

PUT    /api/v1/users/profile
Headers:  Authorization: Bearer <token>
Request:  { name, email }
Response: { user }

POST   /api/v1/users/verify-email
Request:  { token }
Response: { success: true }
```

## Database Schema

### PostgreSQL Tables

```sql
-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    name VARCHAR(255),
    email_verified BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    anonymous_id VARCHAR(255) UNIQUE,
    status VARCHAR(20) NOT NULL DEFAULT 'active'
);

-- Licenses table
CREATE TABLE licenses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    license_key VARCHAR(50) UNIQUE NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    tier VARCHAR(20) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP,
    max_devices INTEGER NOT NULL,
    max_monthly_bytes BIGINT,
    subscription_id VARCHAR(255),
    status VARCHAR(20) NOT NULL DEFAULT 'active'
);

CREATE INDEX idx_licenses_user_id ON licenses(user_id);
CREATE INDEX idx_licenses_key ON licenses(license_key);

-- License devices table
CREATE TABLE license_devices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    license_id UUID REFERENCES licenses(id) ON DELETE CASCADE,
    device_id VARCHAR(255) NOT NULL,
    device_name VARCHAR(255),
    device_fingerprint TEXT,
    activated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_seen TIMESTAMP NOT NULL DEFAULT NOW(),
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    UNIQUE(license_id, device_id)
);

CREATE INDEX idx_devices_license_id ON license_devices(license_id);

-- Usage records table
CREATE TABLE usage_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    license_id UUID REFERENCES licenses(id) ON DELETE CASCADE,
    device_id UUID REFERENCES license_devices(id) ON DELETE SET NULL,
    bytes_transferred BIGINT NOT NULL,
    transfer_type VARCHAR(20) NOT NULL,
    recorded_at TIMESTAMP NOT NULL DEFAULT NOW(),
    period_start TIMESTAMP NOT NULL,
    period_end TIMESTAMP NOT NULL
);

CREATE INDEX idx_usage_license_id ON usage_records(license_id);
CREATE INDEX idx_usage_recorded_at ON usage_records(recorded_at);

-- Sessions table (for JWT blacklist/revocation)
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL,
    revoked BOOLEAN DEFAULT FALSE
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_token_hash ON sessions(token_hash);

-- Payment transactions table
CREATE TABLE payment_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    license_id UUID REFERENCES licenses(id) ON DELETE SET NULL,
    stripe_payment_id VARCHAR(255) UNIQUE NOT NULL,
    amount INTEGER NOT NULL,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    status VARCHAR(20) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_transactions_user_id ON payment_transactions(user_id);
```

## Technology Stack

### CLI Client
```toml
[dependencies]
# Core QLTP
qltp-core = { path = "../../crates/qltp-core" }
qltp-compression = { path = "../../crates/qltp-compression" }
qltp-storage = { path = "../../crates/qltp-storage" }
qltp-network = { path = "../../crates/qltp-network" }
qltp-auth = { path = "../../crates/qltp-auth" }
qltp-licensing = { path = "../../crates/qltp-licensing" }

# CLI
clap = { version = "4.4", features = ["derive"] }
indicatif = "0.17"

# HTTP client (for backend API)
reqwest = { version = "0.11", features = ["json"] }

# Async
tokio = { version = "1.35", features = ["full"] }
```

### Backend Server
```toml
[dependencies]
# Web framework
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

# Database
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-rustls", "uuid", "chrono"] }

# Authentication
jsonwebtoken = "9.2"
argon2 = "0.5"

# Payments
stripe-rust = "0.26"

# Email
lettre = "0.11"

# Validation
validator = { version = "0.16", features = ["derive"] }

# QLTP libraries
qltp-auth = { path = "../../crates/qltp-auth" }
qltp-licensing = { path = "../../crates/qltp-licensing" }

# Async
tokio = { version = "1.35", features = ["full"] }
```

## Deployment Architecture

### Development
```
Local Machine:
├── CLI (cargo run --bin qltp)
├── Backend Server (cargo run --bin backend-server)
└── PostgreSQL (Docker container)
```

### Production
```
Cloud Infrastructure:
├── CLI Distribution
│   ├── GitHub Releases (binaries)
│   ├── Homebrew (macOS)
│   ├── apt/yum (Linux)
│   └── Chocolatey (Windows)
│
├── Backend Server
│   ├── Kubernetes Cluster
│   │   ├── Backend Pods (3+ replicas)
│   │   ├── Load Balancer
│   │   └── Auto-scaling
│   ├── PostgreSQL (managed)
│   │   ├── Primary + Replicas
│   │   └── Automated backups
│   └── Redis (session cache)
│
└── Supporting Services
    ├── Stripe (payments)
    ├── SendGrid (emails)
    ├── Cloudflare (CDN, DDoS)
    └── Datadog (monitoring)
```

### Docker Compose (Development)
```yaml
version: '3.8'
services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: qltp
      POSTGRES_USER: qltp
      POSTGRES_PASSWORD: dev_password
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data

  backend:
    build: ./apps/backend-server
    ports:
      - "3000:3000"
    environment:
      DATABASE_URL: postgres://qltp:dev_password@postgres:5432/qltp
      JWT_SECRET: dev_secret
      STRIPE_SECRET_KEY: sk_test_...
    depends_on:
      - postgres

volumes:
  postgres_data:
```

### Kubernetes (Production)
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: qltp-backend
spec:
  replicas: 3
  selector:
    matchLabels:
      app: qltp-backend
  template:
    metadata:
      labels:
        app: qltp-backend
    spec:
      containers:
      - name: backend
        image: qltp/backend-server:latest
        ports:
        - containerPort: 3000
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: qltp-secrets
              key: database-url
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: qltp-secrets
              key: jwt-secret
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
---
apiVersion: v1
kind: Service
metadata:
  name: qltp-backend
spec:
  type: LoadBalancer
  ports:
  - port: 443
    targetPort: 3000
  selector:
    app: qltp-backend
```

## Security Considerations

### CLI Client
- Store JWT tokens securely (OS keychain)
- Validate TLS certificates
- Encrypt local license cache
- Rate limit API calls
- Validate backend responses

### Backend Server
- HTTPS only (TLS 1.3)
- JWT with short expiration (15 min)
- Refresh tokens (7 days)
- Rate limiting per IP/user
- SQL injection prevention (parameterized queries)
- CORS configuration
- Input validation
- Password hashing (Argon2)
- Stripe webhook signature verification

### Database
- Encrypted at rest
- Encrypted in transit
- Regular backups
- Access control (least privilege)
- Audit logging

## Monitoring & Observability

### Metrics
- API request rate/latency
- Database query performance
- Transfer success/failure rates
- License activation rate
- Payment conversion rate
- User registration rate

### Logging
- Structured logging (JSON)
- Log levels (ERROR, WARN, INFO, DEBUG)
- Request/response logging
- Error tracking (Sentry)
- Audit logs (user actions)

### Alerting
- API downtime
- Database connection failures
- High error rates
- Payment failures
- Quota exceeded events

## Scalability

### Backend Server
- Horizontal scaling (add more pods)
- Stateless design (JWT tokens)
- Database connection pooling
- Redis for session caching
- CDN for static assets

### Database
- Read replicas for queries
- Connection pooling
- Query optimization
- Partitioning (usage_records by date)
- Archiving old data

### CLI Client
- No scaling needed (runs on user devices)
- P2P transfers scale naturally
- Backend only handles metadata

## Cost Estimation

### Infrastructure (Monthly)
- Kubernetes cluster: $200-500
- PostgreSQL (managed): $100-300
- Redis: $50-100
- Bandwidth: $0.01/GB (minimal, only API calls)
- Monitoring: $50-100
- **Total**: ~$400-1000/month

### Per-User Costs
- Storage: $0 (no file storage)
- Bandwidth: ~$0.001/user (only API calls)
- Database: ~$0.01/user
- **Total**: ~$0.01/user/month

**Note**: Very low per-user cost because files transfer P2P, not through backend!

## Summary

QLTP is a **hybrid P2P + cloud system**:

✅ **P2P File Transfer**: Direct device-to-device, no cloud relay
✅ **Cloud Management**: Authentication, licensing, payments, analytics
✅ **Scalable**: P2P scales naturally, backend only handles metadata
✅ **Cost-Effective**: No file storage/bandwidth costs
✅ **Secure**: TLS encryption, JWT auth, license validation
✅ **Professional**: Stripe payments, multi-device sync, usage tracking

The backend server is NOT a file transfer server - it's a management API that enables the P2P transfer system to work professionally with proper authentication, licensing, and billing.

---

**Next Steps**: Implement Phase 4 (Backend Server) with this architecture.

**Made with Bob** 🤖