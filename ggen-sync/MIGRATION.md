# Database Migration Generation

The `migrate` module provides automatic database migration generation based on schema changes detected between ontology versions.

## Features

### Breaking Change Detection

The module detects four types of breaking changes:

1. **Type Removed** - A type exists in code but not in ontology
2. **Field Removed** - A field was removed from a type
3. **Field Type Changed** - A field's type was changed
4. **Required Field Added** - A non-nullable field was added to an existing type

Non-breaking changes (e.g., adding optional fields with `Option<T>`) are ignored.

### Multi-Database Support

Supports three database backends with backend-specific SQL generation:

- **SQLite** - Uses SQLite-specific types and syntax
- **PostgreSQL** - Uses PostgreSQL types and ALTER COLUMN syntax
- **MySQL** - Uses MySQL types and MODIFY COLUMN syntax

### SQLx Migration Format

Generated migrations follow the SQLx migration naming convention:

```
migrations/
  20260210120000_drop_table_old_task.up.sql
  20260210120000_drop_table_old_task.down.sql
  20260210120001_drop_column_user_legacy_id.up.sql
  20260210120001_drop_column_user_legacy_id.down.sql
```

Each migration includes:
- Timestamp prefix (`YYYYMMDDHHMMSS`)
- Descriptive name (snake_case)
- Both up and down migrations
- Auto-generated comments

## Usage

### Basic Usage

```rust
use ggen_sync::{
    detect_breaking_changes, generate_migrations, apply_migrations,
    DatabaseBackend, SyncDiff, OntologyNode,
};
use std::collections::HashMap;
use std::path::Path;

// 1. Detect schema differences (from ontology vs code comparison)
let diffs: Vec<SyncDiff> = detect_diffs(&ontology, &code);

// 2. Load current ontology
let ontology: HashMap<String, OntologyNode> = read_ontology(ontology_dir)?;

// 3. Detect breaking changes
let breaking = detect_breaking_changes(&diffs, &ontology);

if breaking.is_empty() {
    println!("No breaking changes - no migrations needed");
    return;
}

// 4. Generate migrations for your database backend
let backend = DatabaseBackend::Postgres;
let migrations = generate_migrations(&breaking, backend);

// 5. Write migration files
for migration in migrations {
    migration.write_to_dir(Path::new("migrations"))?;
}
```

### High-Level API

Use `apply_migrations` for the full workflow:

```rust
use ggen_sync::{apply_migrations, DatabaseBackend};
use std::path::Path;

// Generates and writes all migration files in one call
let written_files = apply_migrations(
    &diffs,
    &ontology,
    DatabaseBackend::Postgres,
    Path::new("migrations"),
)?;

println!("Wrote {} migration files", written_files.len());
```

### Database Backend Selection

```rust
use ggen_sync::DatabaseBackend;

// From string
let backend = DatabaseBackend::from_str("postgres")?;

// Direct construction
let backend = DatabaseBackend::Sqlite;
let backend = DatabaseBackend::Postgres;
let backend = DatabaseBackend::Mysql;
```

Supported backend strings:
- `"sqlite"`, `"sqlite3"` → `DatabaseBackend::Sqlite`
- `"postgres"`, `"postgresql"`, `"pg"` → `DatabaseBackend::Postgres`
- `"mysql"`, `"mariadb"` → `DatabaseBackend::Mysql`

## Type Mapping

### Rust → SQL Type Mapping

The module automatically maps Rust types to SQL types:

| Rust Type | SQLite | PostgreSQL | MySQL |
|-----------|--------|------------|-------|
| `String`, `&str` | `TEXT` | `TEXT` | `TEXT` |
| `bool` | `INTEGER` | `BOOLEAN` | `BOOLEAN` |
| `i8`, `u8` | `INTEGER` | `SMALLINT` | `TINYINT` |
| `i16`, `u16` | `INTEGER` | `SMALLINT` | `SMALLINT` |
| `i32`, `u32` | `INTEGER` | `INTEGER` | `INT` |
| `i64`, `u64` | `INTEGER` | `BIGINT` | `BIGINT` |
| `f32` | `REAL` | `REAL` | `FLOAT` |
| `f64` | `REAL` | `DOUBLE PRECISION` | `DOUBLE` |
| `serde_json::Value` | `TEXT` | `JSONB` | `JSON` |
| Custom types | `TEXT` | `TEXT` | `TEXT` |

### Wrapper Handling

- `Option<T>` → Makes column nullable
- `Vec<T>` → Unwrapped (assumes array/JSON storage)

Example:
- `Option<String>` → `TEXT NULL`
- `String` → `TEXT NOT NULL`
- `Vec<i32>` → Maps as `i32` (assumes JSON array)

## Generated SQL Examples

### Type Removed

```sql
-- UP
DROP TABLE IF EXISTS old_task;

-- DOWN
-- WARNING: Cannot automatically recreate table 'old_task'
-- Original schema is unknown. Manual intervention required.
```

### Field Removed

```sql
-- UP (PostgreSQL)
ALTER TABLE user DROP COLUMN legacy_id;

-- DOWN (PostgreSQL)
ALTER TABLE user ADD COLUMN legacy_id INTEGER NOT NULL;
```

### Field Type Changed

```sql
-- UP (PostgreSQL)
ALTER TABLE user ALTER COLUMN score TYPE DOUBLE PRECISION;
ALTER TABLE user ALTER COLUMN score SET NOT NULL;

-- DOWN (PostgreSQL)
ALTER TABLE user ALTER COLUMN score TYPE INTEGER;
ALTER TABLE user ALTER COLUMN score SET NOT NULL;
```

```sql
-- UP (MySQL)
ALTER TABLE user MODIFY COLUMN score DOUBLE NOT NULL;

-- DOWN (MySQL)
ALTER TABLE user MODIFY COLUMN score INT NOT NULL;
```

```sql
-- UP (SQLite)
-- WARNING: SQLite does not support ALTER COLUMN
-- You must recreate the table to change column type
-- 1. CREATE TABLE user_new (... score DOUBLE PRECISION NOT NULL ...)
-- 2. INSERT INTO user_new SELECT ... FROM user
-- 3. DROP TABLE user
-- 4. ALTER TABLE user_new RENAME TO user
```

### Required Field Added

```sql
-- UP
-- WARNING: Adding required column 'email' to existing table
-- You may need to provide a default value or make it nullable
ALTER TABLE user ADD COLUMN email TEXT NOT NULL;

-- DOWN
ALTER TABLE user DROP COLUMN email;
```

## Breaking Change Detection Logic

### What's Breaking?

| Change Type | Breaking? | Why |
|-------------|-----------|-----|
| Type removed | ✅ Yes | Data loss |
| Field removed | ✅ Yes | Data loss |
| Field type changed | ✅ Yes | May lose precision or fail constraints |
| Required field added | ✅ Yes | Existing rows can't satisfy NOT NULL |
| Optional field added | ❌ No | Can be NULL for existing rows |
| Type added | ❌ No | New table, no data loss |

### Examples

```rust
// BREAKING: Required field added
FieldChange::Added {
    name: "email".to_string(),
    field_type: "String".to_string(), // NOT optional
}

// NON-BREAKING: Optional field added
FieldChange::Added {
    name: "phone".to_string(),
    field_type: "Option<String>".to_string(), // Optional
}

// BREAKING: Type changed
FieldChange::TypeMismatch {
    name: "score".to_string(),
    ontology_type: "f64".to_string(),
    code_type: "i32".to_string(),
}
```

## Integration with ggen-sync Workflow

### Full Workflow

```bash
# 1. Compare ontology vs generated code
ggen-sync sync --ontology ggen/ontology --code a2a-rs/src/generated

# 2. If breaking changes detected, generate migrations
ggen-sync migrate \
  --ontology ggen/ontology \
  --code a2a-rs/src/generated \
  --backend postgres \
  --output migrations/

# 3. Review generated SQL

# 4. Apply migrations
sqlx migrate run
```

### Programmatic Integration

```rust
use ggen_sync::{sync, detect_breaking_changes, apply_migrations};

// Detect all changes
let (ontology, code) = sync(&ontology_dir, &code_dir)?;
let diffs = detect_diffs(&ontology, &code);

// Check for breaking changes
let breaking = detect_breaking_changes(&diffs, &ontology);

if !breaking.is_empty() {
    println!("WARNING: Breaking changes detected!");
    println!("Generating migrations...");

    apply_migrations(
        &diffs,
        &ontology,
        DatabaseBackend::Postgres,
        Path::new("migrations"),
    )?;

    println!("Review migrations before applying!");
}
```

## Testing

The module includes comprehensive tests:

```bash
# Run all migration tests
cargo test -p ggen-sync migrate

# Run specific test
cargo test -p ggen-sync test_detect_breaking_changes

# Run example
cargo run -p ggen-sync --example generate_migration
```

## Limitations

### SQLite

SQLite has limited ALTER TABLE support:
- Cannot change column types (requires table recreation)
- Cannot drop columns in older versions
- Migration includes manual recreation steps as comments

### Type Inference

- Custom types default to `TEXT` (may need manual adjustment)
- Array types (Vec<T>) mapped as underlying type (assumes JSON storage)
- Reference types between entities mapped as TEXT/foreign keys

### Rollback Safety

Down migrations are best-effort:
- Dropping a table: Cannot recreate schema automatically
- Type changes: May lose precision on rollback
- Always review generated SQL before applying

## Best Practices

1. **Review All Migrations** - Auto-generated SQL should always be reviewed
2. **Test on Staging** - Run migrations on staging database first
3. **Data Migration** - Add custom data migration code where needed
4. **Backup First** - Always backup production data before migrations
5. **Version Control** - Commit migration files to version control

## Future Enhancements

- Custom type mapping configuration
- Data transformation scripts
- Migration dependency ordering
- Rollback safety analysis
- Migration testing framework
