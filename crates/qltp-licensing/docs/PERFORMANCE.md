# Performance Guide

## SQLite Performance Benchmarks

The licensing system includes comprehensive benchmarks for SQLite operations. Run them with:

```bash
cargo bench --features sqlite
```

### Benchmark Categories

1. **License Creation** - Measures time to create a new license with SQLite persistence
2. **License Lookup** - Measures time to retrieve a license by key
3. **Usage Recording** - Measures time to record a single transfer
4. **Concurrent Operations** - Tests performance with 10, 50, and 100 concurrent operations
5. **Bulk Operations** - Tests creating and listing 10, 50, and 100 licenses
6. **Quota Check** - Measures quota validation with usage history

### Expected Performance

Based on typical hardware (SSD, modern CPU):

- **License Creation**: ~1-2ms per license
- **License Lookup**: ~0.5-1ms per lookup
- **Usage Recording**: ~1-2ms per record
- **Concurrent Operations (10)**: ~10-20ms total
- **Concurrent Operations (100)**: ~100-200ms total
- **Bulk Operations (100 licenses)**: ~100-200ms total

### Performance Optimization Tips

1. **Use In-Memory SQLite for Testing**
   ```rust
   let store = SqliteLicenseStore::in_memory()?;
   ```

2. **Batch Operations When Possible**
   - Group multiple license creations
   - Batch usage records

3. **Index Usage**
   - The SQLite adapter creates indexes on `key`, `email`, and `license_id`
   - These significantly speed up lookups

4. **Connection Reuse**
   - The current implementation uses `Arc<Mutex<Connection>>`
   - For production, consider connection pooling (see below)

## Connection Pooling for Production

### Current Implementation

The current SQLite adapters use a simple `Arc<Mutex<Connection>>` approach:

```rust
pub struct SqliteLicenseStore {
    conn: Arc<Mutex<Connection>>,
}
```

**Pros:**
- Simple and thread-safe
- Works well for moderate load
- No external dependencies

**Cons:**
- Single connection bottleneck under high concurrency
- Mutex contention with many concurrent operations

### Recommended: Connection Pooling with r2d2

For production deployments with high concurrency, consider using connection pooling:

#### 1. Add Dependencies

```toml
[dependencies]
r2d2 = "0.8"
r2d2_sqlite = "0.23"
```

#### 2. Update Store Implementation

```rust
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub struct SqliteLicenseStore {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteLicenseStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::builder()
            .max_size(10) // 10 connections in pool
            .build(manager)?;
        
        // Initialize schema
        let conn = pool.get()?;
        conn.execute(/* schema SQL */, [])?;
        
        Ok(Self { pool })
    }
    
    pub fn in_memory() -> Result<Self> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder()
            .max_size(10)
            .build(manager)?;
        
        let conn = pool.get()?;
        conn.execute(/* schema SQL */, [])?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl LicenseRepository for SqliteLicenseStore {
    async fn save(&self, license: &License) -> Result<()> {
        let conn = self.pool.get()?;
        // Use conn for operations
        Ok(())
    }
}
```

#### 3. Configuration Options

```rust
let pool = Pool::builder()
    .max_size(10)              // Maximum connections
    .min_idle(Some(2))         // Minimum idle connections
    .connection_timeout(Duration::from_secs(5))
    .idle_timeout(Some(Duration::from_secs(300)))
    .build(manager)?;
```

### When to Use Connection Pooling

**Use connection pooling when:**
- Handling >100 concurrent requests
- Running in a web server or API service
- Need to minimize connection overhead
- Want better resource management

**Stick with current implementation when:**
- CLI tool or desktop application
- Low to moderate concurrency (<50 concurrent operations)
- Simplicity is preferred over maximum performance

### Performance Comparison

| Scenario | Current (Mutex) | With Pool (10 conns) |
|----------|----------------|---------------------|
| 10 concurrent ops | ~10-20ms | ~10-15ms |
| 50 concurrent ops | ~50-100ms | ~30-50ms |
| 100 concurrent ops | ~100-200ms | ~50-100ms |
| 500 concurrent ops | ~500-1000ms | ~200-400ms |

## Monitoring and Profiling

### Enable Tracing

The licensing system uses `tracing` for observability:

```rust
use tracing_subscriber;

tracing_subscriber::fmt::init();
```

### Key Metrics to Monitor

1. **License Operations**
   - Creation rate
   - Lookup latency
   - Update frequency

2. **Usage Tracking**
   - Record insertion rate
   - Query performance
   - Storage growth

3. **Database**
   - Connection pool utilization
   - Query execution time
   - Lock contention

### Profiling Tools

- **cargo flamegraph**: CPU profiling
- **criterion**: Benchmark comparisons
- **tracing-chrome**: Timeline visualization

## Scaling Considerations

### Horizontal Scaling

For distributed deployments:

1. **Shared Database**
   - Use PostgreSQL or MySQL instead of SQLite
   - Implement connection pooling per instance
   - Consider read replicas for lookups

2. **Caching Layer**
   - Add Redis for frequently accessed licenses
   - Cache quota calculations
   - Implement cache invalidation strategy

3. **Sharding**
   - Shard by user ID or license key prefix
   - Use consistent hashing
   - Implement cross-shard queries carefully

### Vertical Scaling

For single-instance optimization:

1. **Increase Connection Pool Size**
   - Match to CPU core count
   - Monitor connection utilization
   - Adjust based on workload

2. **Optimize Queries**
   - Add indexes for common queries
   - Use prepared statements
   - Batch operations when possible

3. **Hardware Upgrades**
   - SSD for database storage
   - More RAM for caching
   - Faster CPU for concurrent operations

## Best Practices

1. **Always use indexes** for frequently queried fields
2. **Batch operations** when creating multiple licenses
3. **Monitor performance** in production
4. **Use connection pooling** for high-concurrency scenarios
5. **Profile before optimizing** - measure first, optimize second
6. **Test under load** before deploying to production

## Further Reading

- [SQLite Performance Tuning](https://www.sqlite.org/optoverview.html)
- [r2d2 Connection Pooling](https://docs.rs/r2d2/)
- [Rust Async Performance](https://tokio.rs/tokio/tutorial/async)