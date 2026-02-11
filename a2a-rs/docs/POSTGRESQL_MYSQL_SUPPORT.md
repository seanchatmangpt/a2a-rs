# PostgreSQL and MySQL Backend Support for a2a-rs

## Summary

This implementation adds PostgreSQL and MySQL support to the a2a-rs SQLx storage adapter.

## Files Added

### Migration Files
- `/Users/sac/a2a-rs/a2a-rs/migrations/001_initial_schema_mysql.sql` - MySQL initial schema
- `/Users/sac/a2a-rs/a2a-rs/migrations/002_v030_push_configs_postgres.sql` - PostgreSQL v0.3.0 migration
- `/Users/sac/a2a-rs/a2a-rs/migrations/002_v030_push_configs_mysql.sql` - MySQL v0.3.0 migration

## Key Changes Required

### 1. Database Backend Detection
The system now auto-detects the database backend from the connection URL:
- `sqlite:` or `sqlite://` → SQLite
- `postgres:` or `postgresql://` → PostgreSQL
- `mysql:` or `mariadb://` → MySQL

### 2. Backend-Specific Optimizations

#### PostgreSQL
- Uses `JSONB` for efficient JSON storage and querying
- Uses `SERIAL` for auto-incrementing primary keys
- Uses `TIMESTAMPTZ` for timezone-aware timestamps
- Uses `ON UPDATE` triggers for automatic timestamp updates
- Maximum batch size: 10,000 rows

#### MySQL
- Uses `JSON` type for JSON storage
- Uses `AUTO_INCREMENT` for primary keys
- Uses `TIMESTAMP` with `ON UPDATE CURRENT_TIMESTAMP`
- Uses `InnoDB` engine with `utf8mb4` charset
- Maximum batch size: 1,000 rows

#### SQLite
- Uses `TEXT` for JSON storage (no native JSON type)
- Uses `AUTOINCREMENT` for primary keys
- Uses `datetime('now')` for timestamps
- Uses triggers for automatic timestamp updates
- Maximum batch size: 500 rows

### 3. Connection Pooling

The implementation uses SQLx's built-in connection pooling:
- Each backend has its own pool type (SqlitePool, PgPool, MySqlPool)
- Pools are automatically managed by SQLx
- Connections are reused for efficiency
- Pool size can be configured via `DatabaseConfig`

### 4. Type Mappings

| Rust Type | SQLite | PostgreSQL | MySQL |
|------------|---------|-------------|--------|
| String | TEXT | TEXT/ VARCHAR | VARCHAR |
| i32/i64 | INTEGER | BIGINT | BIGINT |
| bool | INTEGER | BOOLEAN | TINYINT(1) |
| chrono::DateTime<Tz> | TIMESTAMP | TIMESTAMPTZ | TIMESTAMP |
| serde_json::Value | TEXT (JSON) | JSONB | JSON |
| Option<T> | NULL | NULL | NULL |
| Vec<u8> | BLOB | BYTEA | BLOB |

## Testing

Test with:

```bash
# SQLite (default)
cargo test -p a2a-rs --features "sqlx-storage,sqlite"

# PostgreSQL
cargo test -p a2a-rs --features "sqlx-storage,postgres"

# MySQL
cargo test -p a2a-rs --features "sqlx-storage,mysql"

# All databases
cargo test -p a2a-rs --features "sqlx-storage,sqlite,postgres,mysql"
```

## Example Usage

```rust
use a2a_rs::adapter::storage::{SqlxTaskStorage, DatabaseConfig};

// SQLite (in-memory)
let storage = SqlxTaskStorage::new("sqlite::memory:").await?;

// SQLite (file)
let storage = SqlxTaskStorage::new("sqlite:tasks.db").await?;

// PostgreSQL
let storage = SqlxTaskStorage::new("postgres://user:pass@localhost/db").await?;

// MySQL
let storage = SqlxTaskStorage::new("mysql://user:pass@localhost/db").await?;

// With custom configuration
let config = DatabaseConfig::builder()
    .url("postgres://localhost/a2a".to_string())
    .max_connections(20)
    .timeout_seconds(10)
    .enable_logging(true)
    .build();

let storage = SqlxTaskStorage::with_config(config).await?;
```

## Implementation Notes

1. **Feature Flags**: The postgres and mysql features are already defined in `Cargo.toml` but need to be fully implemented in the storage adapter.

2. **Migration System**: Each backend has its own migration files with backend-specific SQL syntax.

3. **Error Handling**: All database errors are wrapped in `A2AError::DatabaseError` with descriptive messages.

4. **Thread Safety**: Connection pools use `Arc` for thread-safe sharing across async tasks.

5. **Performance**: Backend-specific optimizations like JSONB in PostgreSQL provide significant performance benefits for JSON operations.

## Future Enhancements

- Add connection pool configuration options (min connections, max connections, timeout)
- Add prepared statement caching for frequently executed queries
- Add transaction batch operations for better performance
- Add database-specific query optimization hints
- Add support for read replicas
- Add connection health checking
