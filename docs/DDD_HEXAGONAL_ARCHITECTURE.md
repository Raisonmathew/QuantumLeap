# QLTP: Domain-Driven Design & Hexagonal Architecture

## Executive Summary

This document defines the **complete architectural approach** for integrating **licensing, authentication, and access control** into the **QLTP file transfer application** using **Domain-Driven Design (DDD)** principles and **Hexagonal Architecture** (Ports & Adapters) patterns.

**Core Product**: QLTP File Transfer Engine (existing)
**New Features**: Licensing, Authentication, Access Control
**Architecture**: Modular Monolith with Hexagonal Architecture + DDD
**Language**: Rust
**Pattern**: Ports & Adapters with Domain-Driven Design

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Domain-Driven Design Principles](#domain-driven-design-principles)
3. [Hexagonal Architecture](#hexagonal-architecture)
4. [Bounded Contexts](#bounded-contexts)
5. [Complete Code Structure](#complete-code-structure)
6. [Implementation Patterns](#implementation-patterns)
7. [Testing Strategy](#testing-strategy)

---

## Architecture Overview

### The Big Picture

```
┌─────────────────────────────────────────────────────────────────┐
│                    QLTP Application                             │
│              (File Transfer Engine + Licensing)                 │
│                  (Hexagonal Modular Monolith)                   │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                      PRIMARY ADAPTERS                           │
│                    (User Interfaces)                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │  CLI Client  │  │  Desktop App │  │  REST API    │        │
│  │  (qltp send) │  │   (GUI)      │  │  (Optional)  │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    DOMAIN LAYER (The Hexagon)                   │
│                     (Business Logic Core)                       │
│                                                                 │
│  ┌────────────────────────────────────────────────────────┐    │
│  │  CORE DOMAIN: File Transfer (QLTP Engine)             │    │
│  │  • Transfer (Aggregate Root)                           │    │
│  │  • Chunk (Entity)                                      │    │
│  │  • TransferState (Value Object)                        │    │
│  │  • CompressionStrategy (Value Object)                  │    │
│  │  • TransferRepository (Port)                           │    │
│  │  • NetworkProtocol (Port)                              │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌────────────────────────────────────────────────────────┐    │
│  │  SUPPORTING: Authentication (Separate Crate)           │    │
│  │  • AuthToken (Entity)                                  │    │
│  │  • Session (Entity)                                    │    │
│  │  • Credentials (Value Object)                          │    │
│  │  • SessionStore (Port)                                 │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌────────────────────────────────────────────────────────┐    │
│  │  SUPPORTING: Licensing & Access Control                │    │
│  │  • License (Aggregate Root)                            │    │
│  │  • Device (Entity)                                     │    │
│  │  • FeatureFlags (Value Object)                         │    │
│  │  • LicenseRepository (Port)                            │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌────────────────────────────────────────────────────────┐    │
│  │  SUPPORTING: Usage Tracking & Quotas                   │    │
│  │  • UsageRecord (Entity)                                │    │
│  │  • Quota (Value Object)                                │    │
│  │  • UsageRepository (Port)                              │    │
│  └────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    SECONDARY ADAPTERS                           │
│                  (Infrastructure Layer)                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │  File System │  │  Network     │  │  Local DB    │        │
│  │  (Storage)   │  │  (QUIC/TLS)  │  │  (SQLite)    │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │  License     │  │    Stripe    │  │   Metrics    │        │
│  │  Server API  │  │   Payment    │  │  (Optional)  │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
└─────────────────────────────────────────────────────────────────┘
```

### Key Concepts

**Hexagon (Domain Core)**:
- **QLTP Transfer Engine**: Core file transfer business logic
- **Supporting Domains**: Authentication, Licensing, Usage Tracking
- No dependencies on external frameworks
- Defines ports (traits) for external communication
- Completely testable in isolation

**Ports (Traits)**:
- Interfaces defined by the domain
- Primary ports: User interfaces (CLI, GUI, API)
- Secondary ports: Infrastructure (file system, network, database)

**Adapters (Implementations)**:
- Primary adapters: CLI, Desktop App, REST API
- Secondary adapters: File System, Network (QUIC/TLS), SQLite, License Server
- Implement the ports defined by the domain

**Key Principle**: Licensing and authentication are **supporting features** that enable/restrict access to the **core QLTP transfer functionality**

---

## Domain-Driven Design Principles

### 1. Ubiquitous Language

Use the same terminology across code, documentation, and business discussions:

```rust
// ✅ Good: Uses business language
pub struct License {
    key: LicenseKey,
    tier: LicenseTier,
    status: LicenseStatus,
}

impl License {
    pub fn activate(&mut self, device: Device) -> Result<(), DomainError> {
        if self.is_expired() {
            return Err(DomainError::LicenseExpired);
        }
        if self.devices.len() >= self.tier.max_devices() {
            return Err(DomainError::DeviceLimitReached);
        }
        self.devices.push(device);
        self.status = LicenseStatus::Active;
        Ok(())
    }
}
```

### 2. Entities vs Value Objects

**Entities**: Have identity, mutable, lifecycle

```rust
// Entity: Has identity (id), mutable state
pub struct User {
    id: UserId,
    email: Email,
    password_hash: String,
    created_at: DateTime<Utc>,
}

impl User {
    pub fn change_email(&mut self, new_email: Email) -> Result<(), DomainError> {
        self.email = new_email;
        self.updated_at = Utc::now();
        Ok(())
    }
}
```

**Value Objects**: No identity, immutable, compared by value

```rust
// Value Object: No identity, immutable
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Email(String);

impl Email {
    pub fn new(email: String) -> Result<Self, DomainError> {
        if !email.contains('@') {
            return Err(DomainError::InvalidEmail);
        }
        Ok(Self(email))
    }
}

// Value Object: License tier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseTier {
    Free,
    Pro,
    Team,
    Business,
    Enterprise,
}

impl LicenseTier {
    pub fn max_devices(&self) -> usize {
        match self {
            Self::Free => 1,
            Self::Pro => 3,
            Self::Team => 10,
            Self::Business => 50,
            Self::Enterprise => usize::MAX,
        }
    }
}
```

### 3. Aggregates & Aggregate Roots

**Aggregate**: Cluster of entities and value objects treated as a unit

```rust
// Aggregate Root: License
pub struct License {
    key: LicenseKey,
    tier: LicenseTier,
    status: LicenseStatus,
    devices: Vec<Device>, // Entities owned by aggregate
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

impl License {
    pub fn create(key: LicenseKey, tier: LicenseTier) -> Result<Self, DomainError> {
        Ok(Self {
            key,
            tier,
            status: LicenseStatus::Inactive,
            devices: Vec::new(),
            created_at: Utc::now(),
            expires_at: None,
        })
    }
    
    pub fn activate(&mut self, device: Device) -> Result<(), DomainError> {
        self.validate_activation(&device)?;
        self.devices.push(device);
        self.status = LicenseStatus::Active;
        Ok(())
    }
    
    fn validate_activation(&self, device: &Device) -> Result<(), DomainError> {
        if self.is_expired() {
            return Err(DomainError::LicenseExpired);
        }
        if self.devices.len() >= self.tier.max_devices() {
            return Err(DomainError::DeviceLimitReached);
        }
        Ok(())
    }
}
```

---

## Hexagonal Architecture

### Ports & Adapters Pattern

**Port**: Interface (trait) defined by the domain

```rust
// Port: Defined by domain
pub trait LicenseRepository: Send + Sync {
    async fn save(&self, license: &License) -> Result<(), RepositoryError>;
    async fn find_by_key(&self, key: &LicenseKey) -> Result<Option<License>, RepositoryError>;
    async fn find_by_user(&self, user_id: &UserId) -> Result<Vec<License>, RepositoryError>;
}
```

**Adapter**: Implementation of the port

```rust
// Adapter: PostgreSQL implementation
pub struct PostgresLicenseRepository {
    pool: PgPool,
}

#[async_trait]
impl LicenseRepository for PostgresLicenseRepository {
    async fn save(&self, license: &License) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO licenses (key, tier, status) VALUES ($1, $2, $3)
             ON CONFLICT (key) DO UPDATE SET tier = $2, status = $3",
            license.key().as_str(),
            license.tier().to_string(),
            license.status().to_string(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

// Adapter: In-memory implementation (for testing)
pub struct InMemoryLicenseRepository {
    licenses: Arc<RwLock<HashMap<LicenseKey, License>>>,
}

#[async_trait]
impl LicenseRepository for InMemoryLicenseRepository {
    async fn save(&self, license: &License) -> Result<(), RepositoryError> {
        let mut licenses = self.licenses.write().unwrap();
        licenses.insert(license.key().clone(), license.clone());
        Ok(())
    }
}
```

### Primary vs Secondary Ports

**Primary Ports** (Driving): Application services called by external actors

```rust
// Primary Port: Application Service
pub struct LicenseActivationService {
    license_repo: Arc<dyn LicenseRepository>,
    event_publisher: Arc<dyn EventPublisher>,
}

impl LicenseActivationService {
    pub async fn activate_license(
        &self,
        key: &LicenseKey,
        device_info: DeviceInfo,
    ) -> Result<License, ServiceError> {
        let mut license = self.license_repo
            .find_by_key(key)
            .await?
            .ok_or(ServiceError::LicenseNotFound)?;
        
        let device = Device::new(device_info)?;
        license.activate(device)?;
        
        self.license_repo.save(&license).await?;
        self.event_publisher.publish(DomainEvent::LicenseActivated {
            license_key: key.clone(),
        }).await?;
        
        Ok(license)
    }
}
```

**Secondary Ports** (Driven): External systems called by the application

```rust
// Secondary Port: Payment Gateway
pub trait PaymentGateway: Send + Sync {
    async fn create_checkout_session(
        &self,
        tier: LicenseTier,
        user_email: &Email,
    ) -> Result<CheckoutSession, PaymentError>;
}

// Adapter: Stripe implementation
pub struct StripePaymentGateway {
    client: stripe::Client,
}

#[async_trait]
impl PaymentGateway for StripePaymentGateway {
    async fn create_checkout_session(
        &self,
        tier: LicenseTier,
        user_email: &Email,
    ) -> Result<CheckoutSession, PaymentError> {
        let session = stripe::CheckoutSession::create(
            &self.client,
            stripe::CreateCheckoutSession {
                mode: Some(stripe::CheckoutSessionMode::Subscription),
                customer_email: Some(user_email.as_str()),
                ..Default::default()
            }
        ).await?;
        
        Ok(CheckoutSession {
            id: session.id.to_string(),
            url: session.url.unwrap(),
        })
    }
}
```

---

## Bounded Contexts

### Context Map

```
                    ┌──────────────────────────┐
                    │   CORE DOMAIN            │
                    │   File Transfer (QLTP)   │
                    │                          │
                    │  • Transfer              │
                    │  • Chunk                 │
                    │  • TransferState         │
                    └──────────────────────────┘
                              ▲
                              │ Uses
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  Authentication  │  │    Licensing     │  │  Usage Tracking  │
│  (qltp-auth)     │──│    Context       │──│  Context         │
│  Separate Crate  │  │                  │  │                  │
│  • AuthToken     │  │  • License       │  │  • UsageRecord   │
│  • Session       │  │  • Device        │  │  • Quota         │
│  • Credentials   │  │  • FeatureFlags  │  │  • RateLimit     │
└──────────────────┘  └──────────────────┘  └──────────────────┘
         │                     │                     │
         └─────────────────────┴─────────────────────┘
                              │
                              ▼ Shared Kernel
                        ┌──────────────┐
                        │   UserId     │
                        │   Timestamp  │
                        └──────────────┘
```

**Relationship**:
- **Core Domain** (File Transfer) is the primary business capability
- **Supporting Contexts** (Auth, Licensing, Usage) enable/restrict access to core functionality
- Transfer operations check licensing/auth before proceeding

### Context Integration

```rust
// Core domain: File Transfer
pub mod transfer {
    use crate::licensing::LicenseValidator;
    use qltp_auth::AuthToken;  // From separate qltp-auth crate
    
    pub struct Transfer {
        id: TransferId,
        source: PathBuf,
        destination: PathBuf,
        state: TransferState,
    }
    
    impl Transfer {
        pub async fn initiate(
            source: PathBuf,
            destination: PathBuf,
            auth_token: &AuthToken,
            license_validator: &LicenseValidator,
        ) -> Result<Self, TransferError> {
            // Check authentication
            let user_id = auth_token.user_id()?;
            
            // Check licensing (feature flags & quotas)
            license_validator.validate_transfer(&user_id, source.metadata()?.len()).await?;
            
            // Create transfer
            Ok(Self {
                id: TransferId::generate(),
                source,
                destination,
                state: TransferState::Pending,
            })
        }
    }
}

// Supporting context: Licensing
pub mod licensing {
    use qltp_auth::AuthToken;  // From separate qltp-auth crate
    
    pub struct LicenseValidator {
        license_repo: Arc<dyn LicenseRepository>,
        usage_tracker: Arc<UsageTracker>,
    }
    
    impl LicenseValidator {
        pub async fn validate_transfer(
            &self,
            user_id: &UserId,
            file_size: u64,
        ) -> Result<(), LicenseError> {
            let license = self.license_repo.find_by_user(user_id).await?;
            
            // Check feature flags
            if !license.can_use_feature(Feature::LargeFileTransfer) && file_size > 1_000_000_000 {
                return Err(LicenseError::FeatureNotAvailable);
            }
            
            // Check quota
            if !self.usage_tracker.check_quota(user_id, file_size).await? {
                return Err(LicenseError::QuotaExceeded);
            }
            
            Ok(())
        }
    }
}
```

---

## Complete Code Structure

```
qltp-project/                      # QLTP Application
├── Cargo.toml
├── crates/
│   ├── qltp-core/                 # CORE DOMAIN: File Transfer Engine
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── transfer/          # Transfer aggregate
│   │   │   │   ├── mod.rs
│   │   │   │   ├── transfer.rs    # Aggregate root
│   │   │   │   ├── chunk.rs       # Entity
│   │   │   │   └── state.rs       # Value object
│   │   │   ├── compression/       # Compression strategies
│   │   │   ├── deduplication/
│   │   │   ├── adaptive.rs
│   │   │   └── prefetch.rs
│   │   └── Cargo.toml
│   │
│   ├── qltp-network/              # Network infrastructure
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── protocol.rs
│   │   │   ├── quic.rs
│   │   │   ├── tls.rs
│   │   │   └── codec.rs
│   │   └── Cargo.toml
│   │
│   ├── qltp-auth/                 # 🆕 REFACTORED: Authentication (Separate Crate)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   │
│   │   │   ├── domain/            # Domain layer (DDD)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── token.rs       # AuthToken entity
│   │   │   │   ├── credentials.rs # Credentials value object
│   │   │   │   └── session.rs     # Session entity
│   │   │   │
│   │   │   ├── application/       # Application layer
│   │   │   │   ├── mod.rs
│   │   │   │   └── auth_service.rs # AuthService (was AuthManager)
│   │   │   │
│   │   │   ├── ports/             # Hexagonal architecture ports
│   │   │   │   ├── mod.rs
│   │   │   │   └── session_store.rs # Port for session storage
│   │   │   │
│   │   │   ├── adapters/          # Adapters for ports
│   │   │   │   ├── mod.rs
│   │   │   │   └── memory_store.rs # In-memory session store
│   │   │   │
│   │   │   └── error.rs           # Auth-specific errors
│   │   └── Cargo.toml
│   │
│   ├── qltp-storage/              # State persistence
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   └── resume.rs
│   │   └── Cargo.toml
│   │
│   ├── qltp-licensing/            # 🆕 NEW: Licensing & Access Control
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   │
│   │   │   ├── domain/            # Domain layer
│   │   │   │   ├── license/       # Licensing context
│   │   │   │   │   ├── aggregates/
│   │   │   │   │   │   └── license.rs
│   │   │   │   │   ├── entities/
│   │   │   │   │   │   └── device.rs
│   │   │   │   │   ├── value_objects/
│   │   │   │   │   │   ├── license_key.rs
│   │   │   │   │   │   ├── license_tier.rs
│   │   │   │   │   │   └── feature_flags.rs
│   │   │   │   │   └── ports/
│   │   │   │   │       └── license_repository.rs
│   │   │   │   │
│   │   │   │   └── usage/         # Usage tracking context
│   │   │   │       ├── entities/
│   │   │   │       │   └── usage_record.rs
│   │   │   │       ├── value_objects/
│   │   │   │       │   ├── quota.rs
│   │   │   │       │   └── rate_limit.rs
│   │   │   │       └── ports/
│   │   │   │           └── usage_repository.rs
│   │   │   │
│   │   │   ├── application/       # Use cases
│   │   │   │   ├── activate_license.rs
│   │   │   │   ├── validate_license.rs
│   │   │   │   └── track_usage.rs
│   │   │   │
│   │   │   └── adapters/          # Infrastructure
│   │   │       ├── persistence/
│   │   │       │   └── sqlite/
│   │   │       │       ├── license_repository.rs
│   │   │       │       └── usage_repository.rs
│   │   │       └── license_server/
│   │   │           └── client.rs  # HTTP client for license server
│   │   └── Cargo.toml
│   │
│   └── qltp-compression/          # Compression algorithms
│       └── src/
│
├── apps/
│   └── cli/                       # CLI Application
│       ├── src/
│       │   ├── main.rs
│       │   ├── commands/
│       │   │   ├── send.rs        # File transfer command
│       │   │   ├── receive.rs
│       │   │   ├── license.rs     # 🆕 License management
│       │   │   └── account.rs     # 🆕 Account management
│       │   └── ui/
│       │       └── prompts.rs     # Upgrade prompts
│       └── Cargo.toml
│
└── tests/
    ├── integration/
    │   ├── transfer_tests.rs      # Core transfer tests
    │   ├── licensing_tests.rs     # 🆕 Licensing tests
    │   └── end_to_end_tests.rs
    └── fixtures/
```

**Key Points**:
- **qltp-core**: Core file transfer engine (existing, 122/122 tests passing)
- **qltp-network**: Network layer with existing auth (10/10 tests passing)
- **qltp-licensing**: NEW crate for licensing & access control
- **apps/cli**: CLI with new license/account commands
- Licensing is a **supporting feature** that gates access to core transfer functionality

---

## Implementation Patterns

### Pattern 1: Domain Entity

```rust
// domain/licensing/aggregates/license.rs

pub struct License {
    key: LicenseKey,
    tier: LicenseTier,
    status: LicenseStatus,
    devices: Vec<Device>,
    created_at: DateTime<Utc>,
}

impl License {
    pub fn create(key: LicenseKey, tier: LicenseTier) -> Result<Self, DomainError> {
        Ok(Self {
            key,
            tier,
            status: LicenseStatus::Inactive,
            devices: Vec::new(),
            created_at: Utc::now(),
        })
    }
    
    pub fn activate(&mut self, device: Device) -> Result<(), DomainError> {
        if self.devices.len() >= self.tier.max_devices() {
            return Err(DomainError::DeviceLimitReached);
        }
        self.devices.push(device);
        self.status = LicenseStatus::Active;
        Ok(())
    }
}
```

### Pattern 2: Application Service

```rust
// application/licensing/activate_license.rs

pub struct ActivateLicenseService {
    license_repo: Arc<dyn LicenseRepository>,
    event_publisher: Arc<dyn EventPublisher>,
}

impl ActivateLicenseService {
    pub async fn execute(
        &self,
        request: ActivateLicenseRequest,
    ) -> Result<ActivateLicenseResponse, ServiceError> {
        // Load aggregate
        let mut license = self.license_repo
            .find_by_key(&request.license_key)
            .await?
            .ok_or(ServiceError::LicenseNotFound)?;
        
        // Create device
        let device = Device::new(request.device_name)?;
        
        // Execute business logic
        license.activate(device)?;
        
        // Persist
        self.license_repo.save(&license).await?;
        
        // Publish event
        self.event_publisher.publish(DomainEvent::LicenseActivated {
            license_key: license.key().clone(),
        }).await?;
        
        Ok(ActivateLicenseResponse {
            license_key: license.key().clone(),
            tier: license.tier(),
        })
    }
}
```

### Pattern 3: REST API Adapter

```rust
// adapters/primary/rest/routes/licenses.rs

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/licenses")
            .route("/activate", web::post().to(activate_license))
    );
}

async fn activate_license(
    body: web::Json<ActivateLicenseDto>,
    service: web::Data<ActivateLicenseService>,
) -> HttpResponse {
    let request = ActivateLicenseRequest {
        license_key: LicenseKey::from_str(&body.license_key)?,
        device_name: body.device_name.clone(),
    };
    
    match service.execute(request).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => HttpResponse::BadRequest().json(e),
    }
}
```

---

## Testing Strategy

### 1. Domain Tests (Pure Unit Tests)

```rust
#[test]
fn test_activate_license_success() {
    let mut license = License::create(
        LicenseKey::generate(),
        LicenseTier::Pro,
    ).unwrap();
    
    let device = Device::new("MacBook Pro".to_string()).unwrap();
    let result = license.activate(device);
    
    assert!(result.is_ok());
    assert_eq!(license.status(), LicenseStatus::Active);
}

#[test]

---

## Summary

**QLTP Application Architecture**:

✅ **Core Domain**: File Transfer Engine (QLTP)
- Existing implementation with 122/122 tests passing
- Compression, deduplication, parallel streaming, QUIC, TLS
- Pure business logic for high-speed file transfers

✅ **Supporting Domains**: Licensing, Authentication, Usage Tracking
- Enable/restrict access to core transfer functionality
- Feature flags based on license tier
- Quota management and rate limiting
- Integrated with existing auth system (10/10 tests passing)

✅ **Hexagonal Architecture** (Ports & Adapters)
- Domain layer isolated from infrastructure
- Swappable adapters (SQLite, PostgreSQL, in-memory)
- Testable without external dependencies

✅ **Domain-Driven Design** principles
- Ubiquitous language (Transfer, License, Quota)
- Entities vs Value Objects
- Aggregates with clear boundaries
- Bounded contexts with explicit relationships

### Key Benefits

- **Pure business logic** in domain layer (no framework dependencies)
- **Highly testable** (domain, application, integration layers)
- **Flexible infrastructure** (swap databases, add new protocols)
- **Clear module boundaries** (easy to understand and maintain)
- **Future-proof** (can extract to microservices if needed)
- **Licensing as a feature** (not the core product)

### Implementation Flow

```
User runs: qltp send file.bin
    ↓
CLI checks license & auth
    ↓
License validator checks:
  • Is user authenticated?
  • Does license allow this feature?
  • Is quota available?
    ↓
If valid → Execute transfer (QLTP core engine)
If invalid → Show upgrade prompt
    ↓
Track usage after transfer
```

---

*Last Updated: 2026-04-14*  
*Version: 1.0*  
*Pattern: DDD + Hexagonal Architecture*  
*Focus: QLTP File Transfer with Licensing Support*
fn test_activate_license_device_limit() {
    let mut license = License::create(
        LicenseKey::generate(),
        LicenseTier::Free, // Max 1 device
    ).unwrap();
    
    license.activate(Device::new("Device 1".to_string()).unwrap()).unwrap();
    let result = license.activate(Device::new("Device 2".to_string()).unwrap());
    
    assert!(matches!(result, Err(DomainError::DeviceLimitReached)));
}
```

### 2. Application Service Tests

```rust
#[tokio::test]
async fn test_activate_license_use_case() {
    let license_repo = Arc::new(InMemoryLicenseRepository::new());
    let event_publisher = Arc::new(InMemoryEventPublisher::new());
    
    let service = ActivateLicenseService::new(license_repo.clone(), event_publisher);
    
    let license = License::create(LicenseKey::generate(), LicenseTier::Pro).unwrap();
    license_repo.save(&license).await.unwrap();
    
    let request = ActivateLicenseRequest {
        license_key: license.key().clone(),
        device_name: "Test Device".to_string(),
    };
    
    let result = service.execute(request).await;
    assert!(result.is_ok());
}
```

### 3. Integration Tests

```rust
#[tokio::test]
async fn test_license_activation_api() {
    let app = test::init_service(App::new().configure(configure_routes)).await;
    
    let req = test::TestRequest::post()
        .uri("/api/v1/licenses/activate")
        .set_json(&json!({
            "license_key": "QLTP-PRO-XXXX-XXXX-XXXX",
            "device_name": "Test Device"
        }))
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}
```

---

## Summary

**QLTP License Server Architecture**:

✅ **Modular Monolith** with clear bounded contexts  
✅ **Hexagonal Architecture** (Ports & Adapters)  
✅ **Domain-Driven Design** principles  
✅ **Clean separation** of concerns  
✅ **Highly testable** (domain, application, integration)  
✅ **Future-proof** (easy to extract to microservices)

**Key Benefits**:
- Pure business logic in domain layer
- Swappable adapters (PostgreSQL → MongoDB)
- Testable without infrastructure
- Clear module boundaries
- Easy to understand and maintain

---

*Last Updated: 2026-04-14*  
*Version: 1.0*  
*Pattern: DDD + Hexagonal Architecture*

---

## Authentication Refactoring: Separate Crate

### Overview

Authentication has been **refactored from `qltp-network` into a separate `qltp-auth` crate** to follow proper Domain-Driven Design and Hexagonal Architecture principles.

### Why Separate Authentication?

#### 1. **Separation of Concerns**
- **Before**: Authentication was embedded in the network layer (`qltp-network/src/auth.rs`)
- **After**: Authentication is a separate domain with its own bounded context
- **Benefit**: Network layer focuses on transport, auth focuses on identity & sessions

#### 2. **Reusability**
Authentication can now be used by multiple modules:
- `qltp-network` - For connection authentication
- `qltp-licensing` - For user identity and license validation
- `qltp-cli` - For user login/logout
- Future API server - For API authentication

#### 3. **Clean Architecture**
- **Domain Layer**: Pure business logic (AuthToken, Session, Credentials)
- **Ports**: Interfaces for storage (SessionStore trait)
- **Adapters**: Implementations (MemorySessionStore, future: RedisStore, DatabaseStore)

### Architecture: qltp-auth Crate

```
qltp-auth/
├── src/
│   ├── lib.rs                    # Public API
│   │
│   ├── domain/                   # Domain Layer (DDD)
│   │   ├── mod.rs
│   │   ├── token.rs              # AuthToken entity
│   │   ├── credentials.rs        # Credentials value object
│   │   └── session.rs            # Session entity
│   │
│   ├── application/              # Application Layer
│   │   ├── mod.rs
│   │   └── auth_service.rs       # AuthService (orchestrates domain logic)
│   │
│   ├── ports/                    # Hexagonal Architecture Ports
│   │   ├── mod.rs
│   │   └── session_store.rs      # SessionStore trait (port)
│   │
│   ├── adapters/                 # Adapters (Infrastructure)
│   │   ├── mod.rs
│   │   └── memory_store.rs       # In-memory implementation
│   │
│   └── error.rs                  # Auth-specific errors
│
└── Cargo.toml
```

### Domain-Driven Design Structure

#### Entities (Have Identity)
```rust
// AuthToken - Unique identifier for authentication
pub struct AuthToken(String);

// Session - Represents an active user session
pub struct Session {
    token: AuthToken,
    username: String,
    created_at: SystemTime,
    expires_at: SystemTime,
}
```

#### Value Objects (Immutable, No Identity)
```rust
// Credentials - Username/password pair
pub struct Credentials {
    pub username: String,
    pub password: String,
}
```

#### Services (Orchestrate Domain Logic)
```rust
// AuthService - Manages authentication operations
pub struct AuthService {
    credentials: Arc<RwLock<HashMap<String, String>>>,
    session_store: Arc<dyn SessionStore>,
    session_ttl: Duration,
}
```

### Hexagonal Architecture: Ports & Adapters

#### Port (Interface)
```rust
/// Port for session storage (hexagonal architecture)
pub trait SessionStore: Send + Sync {
    fn save(&self, session: Session) -> Result<()>;
    fn get(&self, token: &AuthToken) -> Result<Option<Session>>;
    fn remove(&self, token: &AuthToken) -> Result<()>;
    fn cleanup_expired(&self) -> Result<usize>;
    fn count(&self) -> Result<usize>;
}
```

#### Adapter (Implementation)
```rust
/// In-memory session store adapter
pub struct MemorySessionStore {
    sessions: Arc<RwLock<HashMap<AuthToken, Session>>>,
}

impl SessionStore for MemorySessionStore {
    fn save(&self, session: Session) -> Result<()> { /* ... */ }
    fn get(&self, token: &AuthToken) -> Result<Option<Session>> { /* ... */ }
    // ... other methods
}
```

**Future Adapters**:
- `RedisSessionStore` - For distributed sessions
- `DatabaseSessionStore` - For persistent sessions
- `JwtSessionStore` - For stateless JWT tokens

### Integration with Other Crates

#### qltp-network Integration
```rust
// qltp-network/Cargo.toml
[dependencies]
qltp-auth = { path = "../qltp-auth" }

// qltp-network/src/lib.rs
pub use qltp_auth::{AuthService, AuthToken, Credentials, SessionInfo};

// Usage in network code
use qltp_auth::{AuthService, AuthToken};

pub struct Connection {
    auth_service: Arc<AuthService>,
    // ...
}
```

#### qltp-licensing Integration
```rust
// qltp-licensing/Cargo.toml
[dependencies]
qltp-auth = { path = "../qltp-auth" }

// qltp-licensing/src/domain/license.rs
use qltp_auth::AuthToken;

pub struct License {
    user_token: AuthToken,  // Link to authenticated user
    // ...
}
```

#### CLI Integration
```rust
// apps/cli/Cargo.toml
[dependencies]
qltp-auth = { path = "../../crates/qltp-auth" }

// apps/cli/src/commands/login.rs
use qltp_auth::{AuthService, Credentials};

pub async fn login(username: String, password: String) -> Result<()> {
    let auth_service = AuthService::new(/* ... */);
    let creds = Credentials::new(username, password);
    let token = auth_service.authenticate(&creds)?;
    // Save token for future use
    Ok(())
}
```

### Benefits of This Architecture

#### 1. **Testability**
```rust
// Easy to mock SessionStore for testing
struct MockSessionStore;
impl SessionStore for MockSessionStore { /* ... */ }

#[test]
fn test_auth_service() {
    let mock_store = Arc::new(MockSessionStore);
    let auth_service = AuthService::new(mock_store, Duration::from_secs(3600));
    // Test without real storage
}
```

#### 2. **Flexibility**
```rust
// Switch storage backends without changing business logic
let auth_service = if cfg!(production) {
    AuthService::new(Arc::new(RedisSessionStore::new()), ttl)
} else {
    AuthService::new(Arc::new(MemorySessionStore::new()), ttl)
};
```

#### 3. **Maintainability**
- Clear separation between domain logic and infrastructure
- Easy to understand and modify
- Changes to storage don't affect domain logic

#### 4. **Extensibility**
Easy to add new features:
- Multi-factor authentication (MFA)
- OAuth/OIDC providers
- Role-based access control (RBAC)
- Session analytics
- Audit logging

### Migration Path

#### Phase 1: Create qltp-auth Crate ✅
- [x] Create directory structure
- [x] Implement domain layer
- [x] Implement ports & adapters
- [x] Move tests from qltp-network

#### Phase 2: Update Dependencies
- [ ] Add qltp-auth to workspace Cargo.toml
- [ ] Update qltp-network to depend on qltp-auth
- [ ] Remove auth.rs from qltp-network
- [ ] Update imports across codebase

#### Phase 3: Integration
- [ ] Update qltp-licensing to use qltp-auth
- [ ] Update CLI to use qltp-auth
- [ ] Add error conversion at boundaries

#### Phase 4: Testing & Validation
- [ ] Run all tests (10/10 should pass)
- [ ] Integration testing
- [ ] Performance testing

### Backward Compatibility

The refactoring maintains backward compatibility:

```rust
// Old code (still works via re-export)
use qltp_network::{AuthManager, AuthToken};

// New code (recommended)
use qltp_auth::{AuthService, AuthToken};

// Type alias for compatibility
pub type AuthManager = AuthService;
```

### Summary

**Before Refactoring**:
```
qltp-network/
└── src/
    └── auth.rs  (391 lines, mixed concerns)
```

**After Refactoring**:
```
qltp-auth/                    # Separate, reusable crate
├── domain/                   # Pure business logic
├── application/              # Use cases
├── ports/                    # Interfaces
└── adapters/                 # Implementations

qltp-network/                 # Focuses on transport
├── protocol.rs
├── quic.rs
└── tls.rs

qltp-licensing/               # Can use auth
└── uses qltp-auth

apps/cli/                     # Can use auth
└── uses qltp-auth
```

**Key Improvements**:
- ✅ Separation of concerns
- ✅ Reusability across modules
- ✅ Hexagonal architecture (ports & adapters)
- ✅ Domain-Driven Design structure
- ✅ Easy to test and extend
- ✅ Backward compatible

---

## Next Steps

1. **Implement qltp-auth crate** following the structure defined above
2. **Create qltp-licensing crate** that depends on qltp-auth
3. **Integrate with CLI** for user authentication
4. **Add Redis adapter** for production session storage
5. **Implement license validation** using auth tokens

For detailed implementation steps, see [`AUTH_REFACTORING_PLAN.md`](AUTH_REFACTORING_PLAN.md).
