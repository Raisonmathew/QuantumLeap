# DDD + Hexagonal Architecture Refactoring

## Overview

Refactoring `qltp-signaling` and `qltp-connection` to follow Domain-Driven Design (DDD) and Hexagonal Architecture principles, matching the pattern used in `qltp-transport`.

## Architecture Layers

### 1. Domain Layer (Core Business Logic)
- **Pure business logic** - No external dependencies
- **Entities** - Objects with identity (Peer, Session, Connection)
- **Value Objects** - Immutable objects (PeerId, SessionId, NatType)
- **Domain Services** - Business logic that doesn't fit in entities
- **Domain Events** - Things that happened in the domain

### 2. Application Layer (Use Cases)
- **Application Services** - Orchestrate domain objects
- **Use Cases** - Specific application operations
- **DTOs** - Data Transfer Objects for boundaries
- **Command/Query handlers** - CQRS pattern

### 3. Ports Layer (Interfaces)
- **Inbound Ports** - Interfaces for driving adapters (API)
- **Outbound Ports** - Interfaces for driven adapters (DB, Network)
- **Repository Interfaces** - Data access abstractions
- **Service Interfaces** - External service abstractions

### 4. Adapters Layer (Infrastructure)
- **Inbound Adapters** - WebSocket, HTTP, CLI
- **Outbound Adapters** - Database, Network, File System
- **Protocol Adapters** - Message encoding/decoding

### 5. Infrastructure Layer (Technical Concerns)
- **Logging** - Tracing, metrics
- **Configuration** - Settings management
- **Utilities** - Helper functions

---

## qltp-signaling Refactoring

### Current Structure
```
src/
├── lib.rs (426 lines - mixed concerns)
├── types.rs (267 lines - domain + infrastructure)
└── protocol.rs (186 lines - protocol + serialization)
```

### Target Structure
```
src/
├── lib.rs (exports)
├── domain/
│   ├── mod.rs
│   ├── peer.rs (Peer entity)
│   ├── session.rs (Session entity)
│   ├── peer_id.rs (PeerId value object)
│   ├── session_id.rs (SessionId value object)
│   ├── nat_type.rs (NatType value object)
│   ├── ice_candidate.rs (IceCandidate value object)
│   ├── capabilities.rs (PeerCapabilities value object)
│   └── events.rs (Domain events)
├── application/
│   ├── mod.rs
│   ├── peer_service.rs (Peer registration/discovery)
│   ├── session_service.rs (Session management)
│   ├── signaling_service.rs (Main orchestrator)
│   └── dtos.rs (Data transfer objects)
├── ports/
│   ├── mod.rs
│   ├── peer_repository.rs (Peer storage interface)
│   ├── session_repository.rs (Session storage interface)
│   └── message_sender.rs (Message sending interface)
├── adapters/
│   ├── mod.rs
│   ├── websocket.rs (WebSocket adapter)
│   ├── in_memory_peer_repo.rs (In-memory peer storage)
│   └── in_memory_session_repo.rs (In-memory session storage)
├── infrastructure/
│   ├── mod.rs
│   ├── config.rs (Configuration)
│   └── metrics.rs (Metrics collection)
└── protocol/
    ├── mod.rs
    ├── messages.rs (Protocol messages)
    └── codec.rs (Serialization)
```

### Refactoring Steps

#### Step 1: Domain Layer
1. Extract `PeerId`, `SessionId` as value objects
2. Extract `NatType`, `IceCandidate`, `PeerCapabilities` as value objects
3. Create `Peer` entity with business logic
4. Create `Session` entity with state machine
5. Define domain events (PeerRegistered, SessionCreated, etc.)

#### Step 2: Application Layer
6. Create `PeerService` for peer operations
7. Create `SessionService` for session operations
8. Create `SignalingService` as main orchestrator
9. Define DTOs for API boundaries

#### Step 3: Ports Layer
10. Define `PeerRepository` trait
11. Define `SessionRepository` trait
12. Define `MessageSender` trait

#### Step 4: Adapters Layer
13. Implement `WebSocketAdapter` for inbound connections
14. Implement `InMemoryPeerRepository`
15. Implement `InMemorySessionRepository`

#### Step 5: Infrastructure Layer
16. Move configuration to infrastructure
17. Add metrics collection
18. Add health checks

---

## qltp-connection Refactoring

### Current Structure
```
src/
├── lib.rs (232 lines - mixed concerns)
└── strategy.rs (382 lines - strategy + execution)
```

### Target Structure
```
src/
├── lib.rs (exports)
├── domain/
│   ├── mod.rs
│   ├── connection.rs (Connection entity)
│   ├── nat_compatibility.rs (NAT compatibility logic)
│   ├── connection_method.rs (ConnectionMethod value object)
│   ├── connection_strategy.rs (Strategy value object)
│   └── events.rs (Domain events)
├── application/
│   ├── mod.rs
│   ├── connection_service.rs (Connection establishment)
│   ├── strategy_selector.rs (Strategy selection logic)
│   └── dtos.rs (Data transfer objects)
├── ports/
│   ├── mod.rs
│   ├── connection_establisher.rs (Connection interface)
│   ├── nat_detector.rs (NAT detection interface)
│   └── relay_allocator.rs (Relay allocation interface)
├── adapters/
│   ├── mod.rs
│   ├── direct_p2p.rs (Direct P2P adapter)
│   ├── stun_assisted.rs (STUN adapter)
│   └── turn_relay.rs (TURN adapter)
└── infrastructure/
    ├── mod.rs
    ├── config.rs (Configuration)
    └── metrics.rs (Metrics collection)
```

### Refactoring Steps

#### Step 1: Domain Layer
1. Extract `ConnectionMethod` as value object
2. Extract `ConnectionStrategy` as value object
3. Create `Connection` entity
4. Create `NatCompatibility` domain service
5. Define domain events (ConnectionEstablished, etc.)

#### Step 2: Application Layer
6. Create `ConnectionService` for connection operations
7. Create `StrategySelector` for strategy selection
8. Define DTOs for API boundaries

#### Step 3: Ports Layer
9. Define `ConnectionEstablisher` trait
10. Define `NatDetector` trait
11. Define `RelayAllocator` trait

#### Step 4: Adapters Layer
12. Implement `DirectP2PAdapter`
13. Implement `StunAssistedAdapter`
14. Implement `TurnRelayAdapter`

#### Step 5: Infrastructure Layer
15. Move configuration to infrastructure
16. Add metrics collection
17. Add connection monitoring

---

## Benefits of This Architecture

### 1. Separation of Concerns
- Domain logic isolated from infrastructure
- Easy to test business logic
- Clear boundaries between layers

### 2. Dependency Inversion
- Domain doesn't depend on infrastructure
- Infrastructure depends on domain interfaces
- Easy to swap implementations

### 3. Testability
- Domain logic testable without infrastructure
- Mock adapters for integration tests
- Clear test boundaries

### 4. Maintainability
- Changes localized to specific layers
- Easy to understand code organization
- Clear responsibility for each module

### 5. Flexibility
- Easy to add new adapters (HTTP, gRPC, etc.)
- Easy to change storage (Redis, PostgreSQL, etc.)
- Easy to add new features

---

## Implementation Priority

### Phase 1: qltp-signaling (High Priority)
- More complex domain logic
- Multiple entities and relationships
- Critical for relay service

### Phase 2: qltp-connection (Medium Priority)
- Simpler domain logic
- Fewer entities
- Can leverage signaling refactoring

---

## Testing Strategy

### Domain Layer Tests
- Pure unit tests
- No mocks needed
- Test business logic thoroughly

### Application Layer Tests
- Use mock ports
- Test use case orchestration
- Test error handling

### Adapter Tests
- Integration tests
- Test actual implementations
- Test protocol compliance

### End-to-End Tests
- Full system tests
- Test real scenarios
- Test performance

---

## Migration Strategy

### 1. Create New Structure
- Create new directories
- Move code incrementally
- Keep old code working

### 2. Gradual Migration
- Migrate one layer at a time
- Update tests as you go
- Ensure all tests pass

### 3. Remove Old Code
- Once migration complete
- Remove old files
- Update documentation

### 4. Verify
- Run all tests
- Check performance
- Validate functionality

---

## Next Steps

1. ✅ Create this refactoring plan
2. ⏳ Refactor qltp-signaling domain layer
3. ⏳ Refactor qltp-signaling application layer
4. ⏳ Refactor qltp-signaling adapters
5. ⏳ Refactor qltp-connection domain layer
6. ⏳ Refactor qltp-connection application layer
7. ⏳ Refactor qltp-connection adapters
8. ⏳ Update all tests
9. ⏳ Update documentation
10. ⏳ Performance validation
