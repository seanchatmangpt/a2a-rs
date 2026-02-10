# Database Migration Generation

## Overview

Created `ggen-sync/src/migrate.rs` for automatic database migration generation from schema changes.

## Files Created

1. **`ggen-sync/src/migrate.rs`** (752 lines)
   - Breaking change detection
   - SQL migration generation for SQLite, PostgreSQL, MySQL
   - SQLx migration file format support
   - Comprehensive tests

2. **`ggen-sync/examples/generate_migration.rs`**
   - Complete working example showing migration generation
   - Demonstrates all breaking change types
   - Shows multi-backend SQL generation

3. **`ggen-sync/MIGRATION.md`**
   - Complete documentation with examples
   - API reference
   - Type mapping tables
   - Best practices guide

## Dependencies Added

- `chrono = "0.4"` (for timestamp generation in migration filenames)

## Key Types

### `DatabaseBackend` Enum
```rust
pub enum DatabaseBackend {
    Sqlite,
    Postgres,
    Mysql,
}
```

Parses from strings: `"sqlite"`, `"postgres"`, `"mysql"`, etc.

### `BreakingChange` Enum
```rust
pub enum BreakingChange {
    TypeRemoved { type_name: String },
    FieldRemoved { type_name, field_name, field_type },
    FieldTypeChanged { type_name, field_name, old_type, new_type },
    RequiredFieldAdded { type_name, field_name, field_type },
}
```

### `Migration` Struct
```rust
pub struct Migration {
    pub timestamp: String,      // "20260210120000"
    pub description: String,    // "drop_column_user_email"
    pub up_sql: String,
    pub down_sql: String,
}
```

## API Functions

### `detect_breaking_changes(diffs, ontology) -> Vec<BreakingChange>`
Detects breaking schema changes from `SyncDiff` results.

**Breaking changes:**
- Type removed from ontology
- Field removed from type
- Field type changed
- Required field added (non-Option)

**Non-breaking:**
- Optional field added (Option<T>)
- New type added to ontology

### `generate_migrations(breaking_changes, backend) -> Vec<Migration>`
Generates SQL migrations for detected breaking changes.

**Backend-specific SQL:**
- SQLite: Uses `TEXT`, `INTEGER`, `REAL`; notes about ALTER COLUMN limitations
- PostgreSQL: Uses `TEXT`, `INTEGER`, `BIGINT`, `JSONB`; proper ALTER COLUMN syntax
- MySQL: Uses `TEXT`, `INT`, `BIGINT`, `JSON`; MODIFY COLUMN syntax

### `apply_migrations(diffs, ontology, backend, migrations_dir) -> Result<Vec<PathBuf>>`
High-level API: detects breaking changes, generates migrations, writes files.

Returns list of written migration file paths.

## Type Mapping Pattern

```rust
Rust Type         → SQLite  → PostgreSQL       → MySQL
String            → TEXT    → TEXT             → TEXT
bool              → INTEGER → BOOLEAN          → BOOLEAN
i32               → INTEGER → INTEGER          → INT
i64               → INTEGER → BIGINT           → BIGINT
f64               → REAL    → DOUBLE PRECISION → DOUBLE
serde_json::Value → TEXT    → JSONB           → JSON
Option<T>         → (nullable column)
Vec<T>            → (array/JSON storage, mapped as T)
```

## Migration File Format

SQLx format: `<timestamp>_<description>.[up|down].sql`

Example:
```
migrations/
  20260210120000_drop_table_old_task.up.sql
  20260210120000_drop_table_old_task.down.sql
  20260210120001_alter_column_user_score.up.sql
  20260210120001_alter_column_user_score.down.sql
```

Each file includes:
- Auto-generated comment header
- Timestamp in ISO 8601 format
- Migration SQL

## SQL Generation Examples

### PostgreSQL - Type Changed
```sql
-- UP
ALTER TABLE user ALTER COLUMN score TYPE DOUBLE PRECISION;
ALTER TABLE user ALTER COLUMN score SET NOT NULL;

-- DOWN
ALTER TABLE user ALTER COLUMN score TYPE INTEGER;
ALTER TABLE user ALTER COLUMN score SET NOT NULL;
```

### MySQL - Field Removed
```sql
-- UP
ALTER TABLE user DROP COLUMN legacy_id;

-- DOWN
ALTER TABLE user ADD COLUMN legacy_id INT NOT NULL;
```

### SQLite - Type Changed (with warning)
```sql
-- WARNING: SQLite does not support ALTER COLUMN
-- You must recreate the table to change column type
-- 1. CREATE TABLE user_new (... score REAL NOT NULL ...)
-- 2. INSERT INTO user_new SELECT ... FROM user
-- 3. DROP TABLE user
-- 4. ALTER TABLE user_new RENAME TO user
```

## Testing

Comprehensive test suite includes:
- `test_database_backend_from_str()` - Backend parsing
- `test_sql_type_mapping()` - Rust → SQL type conversion
- `test_strip_wrappers()` - Option<>/Vec<> unwrapping
- `test_is_optional()` - Nullability detection
- `test_to_snake_case()` - Name conversion
- `test_detect_breaking_changes_*()` - Breaking change detection
- `test_generate_migration_*()` - SQL generation

## Integration Pattern

```rust
use ggen_sync::{detect_diffs, detect_breaking_changes, apply_migrations};

// 1. Detect schema drift
let diffs = detect_diffs(&ontology, &code);

// 2. Check for breaking changes
let breaking = detect_breaking_changes(&diffs, &ontology);

if !breaking.is_empty() {
    // 3. Generate and write migrations
    apply_migrations(
        &diffs,
        &ontology,
        DatabaseBackend::Postgres,
        Path::new("migrations"),
    )?;
}
```

## Common Patterns

### Case Conversion
- Type names: `AgentCard` → `agent_card` (table names)
- Field names: `myField` → `my_field` (column names)
- Uses `to_snake_case()` helper

### Wrapper Stripping
- `Option<String>` → `String` (base type) + nullable flag
- `Vec<String>` → `String` (base type) + array flag
- Recursive: `Option<Vec<String>>` → `String` + nullable + array

### Lifetime Annotations
The `strip_wrapper<'a>(s: &'a str, wrapper: &str) -> Option<&'a str>` pattern ensures returned string slices have the correct lifetime tied to the input string.

## Limitations

1. **SQLite ALTER TABLE**: Limited support, generates manual recreation steps
2. **Custom Types**: Default to TEXT, may need manual SQL adjustment
3. **Rollback Safety**: Down migrations are best-effort (e.g., can't recreate dropped table schema)
4. **Data Migration**: Only generates schema changes, not data transformation

## Future Enhancements

- Custom type mapping configuration file
- Data migration script generation
- Migration dependency ordering
- Schema compatibility analysis
- Migration testing framework
