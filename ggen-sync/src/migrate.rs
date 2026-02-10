//! Schema migration generation
//!
//! Detects breaking changes between schema versions and generates SQLx-compatible
//! migration files for database schema updates.

use crate::types::{FieldChange, OntologyNode, SyncDiff};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid migration timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("Unsupported database backend: {0}")]
    UnsupportedBackend(String),

    #[error("Type not found in ontology: {0}")]
    TypeNotFound(String),
}

type Result<T> = std::result::Result<T, MigrationError>;

/// Database backend for migration generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseBackend {
    Sqlite,
    Postgres,
    Mysql,
}

impl DatabaseBackend {
    /// Parse backend from string
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "sqlite" | "sqlite3" => Ok(Self::Sqlite),
            "postgres" | "postgresql" | "pg" => Ok(Self::Postgres),
            "mysql" | "mariadb" => Ok(Self::Mysql),
            _ => Err(MigrationError::UnsupportedBackend(s.to_string())),
        }
    }

    /// Get SQL type for a Rust type
    fn sql_type(&self, rust_type: &str) -> String {
        // Strip Option<> and Vec<> wrappers
        let base_type = strip_wrappers(rust_type);

        match (self, base_type) {
            // String types
            (DatabaseBackend::Sqlite, "String" | "str" | "&str") => "TEXT".to_string(),
            (DatabaseBackend::Postgres, "String" | "str" | "&str") => "TEXT".to_string(),
            (DatabaseBackend::Mysql, "String" | "str" | "&str") => "TEXT".to_string(),

            // Boolean
            (DatabaseBackend::Sqlite, "bool") => "INTEGER".to_string(), // SQLite uses 0/1
            (DatabaseBackend::Postgres, "bool") => "BOOLEAN".to_string(),
            (DatabaseBackend::Mysql, "bool") => "BOOLEAN".to_string(),

            // Integers
            (DatabaseBackend::Sqlite, "i8" | "i16" | "i32" | "u8" | "u16") => "INTEGER".to_string(),
            (DatabaseBackend::Sqlite, "i64" | "u32" | "u64" | "isize" | "usize") => {
                "INTEGER".to_string()
            }
            (DatabaseBackend::Postgres, "i8" | "u8") => "SMALLINT".to_string(),
            (DatabaseBackend::Postgres, "i16" | "u16") => "SMALLINT".to_string(),
            (DatabaseBackend::Postgres, "i32" | "u32") => "INTEGER".to_string(),
            (DatabaseBackend::Postgres, "i64" | "u64" | "isize" | "usize") => "BIGINT".to_string(),
            (DatabaseBackend::Mysql, "i8" | "u8") => "TINYINT".to_string(),
            (DatabaseBackend::Mysql, "i16" | "u16") => "SMALLINT".to_string(),
            (DatabaseBackend::Mysql, "i32" | "u32") => "INT".to_string(),
            (DatabaseBackend::Mysql, "i64" | "u64" | "isize" | "usize") => "BIGINT".to_string(),

            // Floats
            (DatabaseBackend::Sqlite, "f32" | "f64") => "REAL".to_string(),
            (DatabaseBackend::Postgres, "f32") => "REAL".to_string(),
            (DatabaseBackend::Postgres, "f64") => "DOUBLE PRECISION".to_string(),
            (DatabaseBackend::Mysql, "f32") => "FLOAT".to_string(),
            (DatabaseBackend::Mysql, "f64") => "DOUBLE".to_string(),

            // JSON
            (DatabaseBackend::Sqlite, "serde_json::Value" | "Value") => "TEXT".to_string(),
            (DatabaseBackend::Postgres, "serde_json::Value" | "Value") => "JSONB".to_string(),
            (DatabaseBackend::Mysql, "serde_json::Value" | "Value") => "JSON".to_string(),

            // Default: assume it's a foreign key or JSON
            (DatabaseBackend::Sqlite, _) => "TEXT".to_string(),
            (DatabaseBackend::Postgres, _) => "TEXT".to_string(),
            (DatabaseBackend::Mysql, _) => "TEXT".to_string(),
        }
    }
}

/// Strip Option<> and Vec<> wrappers from a type string
fn strip_wrappers(rust_type: &str) -> &str {
    let mut t = rust_type.trim();

    // Strip Option<>
    if let Some(inner) = strip_wrapper(t, "Option") {
        t = inner;
    }

    // Strip Vec<>
    if let Some(inner) = strip_wrapper(t, "Vec") {
        t = inner;
    }

    t
}

/// Strip a single wrapper like Option<T> or Vec<T>
fn strip_wrapper<'a>(s: &'a str, wrapper: &str) -> Option<&'a str> {
    let s = s.trim();
    if s.starts_with(wrapper) && s.ends_with('>') {
        let start = wrapper.len();
        if s.as_bytes().get(start) == Some(&b'<') {
            let inner = &s[start + 1..s.len() - 1];
            return Some(inner.trim());
        }
    }
    None
}

/// Check if a type is Option<T>
fn is_optional(rust_type: &str) -> bool {
    rust_type.trim().starts_with("Option<")
}

/// A breaking change detected in schema
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakingChange {
    /// Type removed from ontology (exists in code but not in ontology)
    TypeRemoved { type_name: String },

    /// Field removed from type
    FieldRemoved {
        type_name: String,
        field_name: String,
        field_type: String,
    },

    /// Field type changed
    FieldTypeChanged {
        type_name: String,
        field_name: String,
        old_type: String,
        new_type: String,
    },

    /// Required field added (non-nullable without default)
    RequiredFieldAdded {
        type_name: String,
        field_name: String,
        field_type: String,
    },
}

/// Detect breaking changes from sync diffs
pub fn detect_breaking_changes(
    diffs: &[SyncDiff],
    _ontology: &HashMap<String, OntologyNode>,
) -> Vec<BreakingChange> {
    let mut breaking = Vec::new();

    for diff in diffs {
        match diff {
            // Type exists in ontology but not in code - this is forward sync, not breaking
            SyncDiff::Added { .. } => {}

            // Type exists in code but not in ontology - BREAKING
            SyncDiff::Removed { type_name } => {
                breaking.push(BreakingChange::TypeRemoved {
                    type_name: type_name.clone(),
                });
            }

            // Type exists in both but has field differences
            SyncDiff::Modified {
                type_name,
                field_changes,
            } => {
                for field_change in field_changes {
                    match field_change {
                        // Field exists in ontology but not in code
                        // This is BREAKING if the field is required
                        FieldChange::Added { name, field_type } => {
                            // Check if the field is required (not Option<T>)
                            if !is_optional(field_type) {
                                breaking.push(BreakingChange::RequiredFieldAdded {
                                    type_name: type_name.clone(),
                                    field_name: name.clone(),
                                    field_type: field_type.clone(),
                                });
                            }
                        }

                        // Field exists in code but not in ontology - BREAKING
                        FieldChange::Removed { name, field_type } => {
                            breaking.push(BreakingChange::FieldRemoved {
                                type_name: type_name.clone(),
                                field_name: name.clone(),
                                field_type: field_type.clone(),
                            });
                        }

                        // Field type mismatch - BREAKING
                        FieldChange::TypeMismatch {
                            name,
                            ontology_type,
                            code_type,
                        } => {
                            breaking.push(BreakingChange::FieldTypeChanged {
                                type_name: type_name.clone(),
                                field_name: name.clone(),
                                old_type: code_type.clone(),
                                new_type: ontology_type.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    breaking
}

/// Migration file pair (up and down SQL)
#[derive(Debug)]
pub struct Migration {
    pub timestamp: String,
    pub description: String,
    pub up_sql: String,
    pub down_sql: String,
}

impl Migration {
    /// Create a new migration with the current timestamp
    pub fn new(description: impl Into<String>, up_sql: String, down_sql: String) -> Self {
        let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
        Self {
            timestamp,
            description: description.into(),
            up_sql,
            down_sql,
        }
    }

    /// Get the up migration file name
    pub fn up_filename(&self) -> String {
        format!("{}_{}.up.sql", self.timestamp, self.description)
    }

    /// Get the down migration file name
    pub fn down_filename(&self) -> String {
        format!("{}_{}.down.sql", self.timestamp, self.description)
    }

    /// Write migration files to a directory
    pub fn write_to_dir(&self, migrations_dir: &Path) -> Result<()> {
        // Create migrations directory if it doesn't exist
        fs::create_dir_all(migrations_dir)?;

        // Write up migration
        let up_path = migrations_dir.join(self.up_filename());
        let mut up_file = File::create(&up_path)?;
        writeln!(up_file, "-- Migration: {}", self.description)?;
        writeln!(up_file, "-- Generated: {}", chrono::Utc::now().to_rfc3339())?;
        writeln!(up_file)?;
        write!(up_file, "{}", self.up_sql)?;
        println!("  Wrote: {}", up_path.display());

        // Write down migration
        let down_path = migrations_dir.join(self.down_filename());
        let mut down_file = File::create(&down_path)?;
        writeln!(down_file, "-- Migration rollback: {}", self.description)?;
        writeln!(
            down_file,
            "-- Generated: {}",
            chrono::Utc::now().to_rfc3339()
        )?;
        writeln!(down_file)?;
        write!(down_file, "{}", self.down_sql)?;
        println!("  Wrote: {}", down_path.display());

        Ok(())
    }
}

/// Generate SQL migrations from breaking changes
pub fn generate_migrations(
    breaking_changes: &[BreakingChange],
    backend: DatabaseBackend,
) -> Vec<Migration> {
    let mut migrations = Vec::new();

    for change in breaking_changes {
        match change {
            BreakingChange::TypeRemoved { type_name } => {
                let table_name = to_table_name(type_name);
                let description = format!("drop_table_{}", table_name);

                let up_sql = format!("DROP TABLE IF EXISTS {};\n", table_name);

                // Down migration: recreate table (but we don't know the schema)
                let down_sql = format!(
                    "-- WARNING: Cannot automatically recreate table '{}'\n\
                     -- Original schema is unknown. Manual intervention required.\n",
                    table_name
                );

                migrations.push(Migration::new(description, up_sql, down_sql));
            }

            BreakingChange::FieldRemoved {
                type_name,
                field_name,
                field_type,
            } => {
                let table_name = to_table_name(type_name);
                let column_name = to_column_name(field_name);
                let description = format!("drop_column_{}_{}", table_name, column_name);

                let up_sql = format!("ALTER TABLE {} DROP COLUMN {};\n", table_name, column_name);

                // Down migration: add column back
                let sql_type = backend.sql_type(field_type);
                let nullable = if is_optional(field_type) {
                    ""
                } else {
                    " NOT NULL"
                };

                let down_sql = format!(
                    "ALTER TABLE {} ADD COLUMN {} {}{};\n",
                    table_name, column_name, sql_type, nullable
                );

                migrations.push(Migration::new(description, up_sql, down_sql));
            }

            BreakingChange::FieldTypeChanged {
                type_name,
                field_name,
                old_type,
                new_type,
            } => {
                let table_name = to_table_name(type_name);
                let column_name = to_column_name(field_name);
                let description = format!("alter_column_{}_{}", table_name, column_name);

                let new_sql_type = backend.sql_type(new_type);
                let old_sql_type = backend.sql_type(old_type);

                let up_sql = generate_alter_column_sql(
                    backend,
                    &table_name,
                    &column_name,
                    &new_sql_type,
                    is_optional(new_type),
                );

                let down_sql = generate_alter_column_sql(
                    backend,
                    &table_name,
                    &column_name,
                    &old_sql_type,
                    is_optional(old_type),
                );

                migrations.push(Migration::new(description, up_sql, down_sql));
            }

            BreakingChange::RequiredFieldAdded {
                type_name,
                field_name,
                field_type,
            } => {
                let table_name = to_table_name(type_name);
                let column_name = to_column_name(field_name);
                let description = format!("add_column_{}_{}", table_name, column_name);

                let sql_type = backend.sql_type(field_type);

                // For required fields, we need a default value
                let up_sql = format!(
                    "-- WARNING: Adding required column '{}' to existing table\n\
                     -- You may need to provide a default value or make it nullable\n\
                     ALTER TABLE {} ADD COLUMN {} {} NOT NULL;\n",
                    column_name, table_name, column_name, sql_type
                );

                let down_sql = format!("ALTER TABLE {} DROP COLUMN {};\n", table_name, column_name);

                migrations.push(Migration::new(description, up_sql, down_sql));
            }
        }
    }

    migrations
}

/// Generate ALTER COLUMN SQL (backend-specific syntax)
fn generate_alter_column_sql(
    backend: DatabaseBackend,
    table: &str,
    column: &str,
    sql_type: &str,
    nullable: bool,
) -> String {
    let null_constraint = if nullable { "" } else { " NOT NULL" };

    match backend {
        DatabaseBackend::Sqlite => {
            // SQLite doesn't support ALTER COLUMN, requires table recreation
            format!(
                "-- WARNING: SQLite does not support ALTER COLUMN\n\
                 -- You must recreate the table to change column type\n\
                 -- 1. CREATE TABLE {}_new (... {} {}{} ...)\n\
                 -- 2. INSERT INTO {}_new SELECT ... FROM {}\n\
                 -- 3. DROP TABLE {}\n\
                 -- 4. ALTER TABLE {}_new RENAME TO {}\n",
                table, column, sql_type, null_constraint, table, table, table, table, table
            )
        }
        DatabaseBackend::Postgres => {
            let mut sql = format!(
                "ALTER TABLE {} ALTER COLUMN {} TYPE {};\n",
                table, column, sql_type
            );

            // Handle nullability separately in PostgreSQL
            if nullable {
                sql.push_str(&format!(
                    "ALTER TABLE {} ALTER COLUMN {} DROP NOT NULL;\n",
                    table, column
                ));
            } else {
                sql.push_str(&format!(
                    "ALTER TABLE {} ALTER COLUMN {} SET NOT NULL;\n",
                    table, column
                ));
            }

            sql
        }
        DatabaseBackend::Mysql => {
            format!(
                "ALTER TABLE {} MODIFY COLUMN {} {}{};\n",
                table, column, sql_type, null_constraint
            )
        }
    }
}

/// Convert type name to table name (snake_case)
fn to_table_name(type_name: &str) -> String {
    to_snake_case(type_name)
}

/// Convert field name to column name (snake_case)
fn to_column_name(field_name: &str) -> String {
    to_snake_case(field_name)
}

/// Convert PascalCase/camelCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_upper = false;

    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 && !prev_upper {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
            prev_upper = true;
        } else {
            result.push(ch);
            prev_upper = false;
        }
    }

    result
}

/// Apply migrations: generate migration files from sync diffs
///
/// # Arguments
/// * `diffs` - Detected differences between ontology and code
/// * `ontology` - Current ontology nodes (for field type lookup)
/// * `backend` - Database backend (sqlite, postgres, mysql)
/// * `migrations_dir` - Directory to write migration files
pub fn apply_migrations(
    diffs: &[SyncDiff],
    ontology: &HashMap<String, OntologyNode>,
    backend: DatabaseBackend,
    migrations_dir: &Path,
) -> Result<Vec<PathBuf>> {
    // Detect breaking changes
    let breaking = detect_breaking_changes(diffs, ontology);

    if breaking.is_empty() {
        println!("No breaking changes detected - no migrations needed");
        return Ok(Vec::new());
    }

    println!("Detected {} breaking change(s):", breaking.len());
    for change in &breaking {
        println!("  - {:?}", change);
    }

    // Generate migrations
    let migrations = generate_migrations(&breaking, backend);

    println!("\nGenerating {} migration(s):", migrations.len());

    let mut written_files = Vec::new();

    for migration in &migrations {
        migration.write_to_dir(migrations_dir)?;
        written_files.push(migrations_dir.join(migration.up_filename()));
        written_files.push(migrations_dir.join(migration.down_filename()));
    }

    println!("\nMigrations written to: {}", migrations_dir.display());

    Ok(written_files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FieldDef, OntologyNode};

    #[test]
    fn test_database_backend_from_str() {
        assert_eq!(
            DatabaseBackend::from_str("sqlite").unwrap(),
            DatabaseBackend::Sqlite
        );
        assert_eq!(
            DatabaseBackend::from_str("postgres").unwrap(),
            DatabaseBackend::Postgres
        );
        assert_eq!(
            DatabaseBackend::from_str("mysql").unwrap(),
            DatabaseBackend::Mysql
        );
        assert!(DatabaseBackend::from_str("invalid").is_err());
    }

    #[test]
    fn test_sql_type_mapping() {
        let sqlite = DatabaseBackend::Sqlite;
        assert_eq!(sqlite.sql_type("String"), "TEXT");
        assert_eq!(sqlite.sql_type("bool"), "INTEGER");
        assert_eq!(sqlite.sql_type("i32"), "INTEGER");
        assert_eq!(sqlite.sql_type("f64"), "REAL");

        let pg = DatabaseBackend::Postgres;
        assert_eq!(pg.sql_type("String"), "TEXT");
        assert_eq!(pg.sql_type("bool"), "BOOLEAN");
        assert_eq!(pg.sql_type("i32"), "INTEGER");
        assert_eq!(pg.sql_type("f64"), "DOUBLE PRECISION");
    }

    #[test]
    fn test_strip_wrappers() {
        assert_eq!(strip_wrappers("String"), "String");
        assert_eq!(strip_wrappers("Option<String>"), "String");
        assert_eq!(strip_wrappers("Vec<String>"), "String");
        assert_eq!(strip_wrappers("Option<Vec<String>>"), "Vec<String>");
    }

    #[test]
    fn test_is_optional() {
        assert!(!is_optional("String"));
        assert!(is_optional("Option<String>"));
        assert!(!is_optional("Vec<String>"));
        assert!(is_optional("Option<Vec<String>>"));
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("AgentCard"), "agent_card");
        assert_eq!(to_snake_case("HTTPClient"), "h_t_t_p_client");
        assert_eq!(to_snake_case("myField"), "my_field");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn test_detect_breaking_changes_type_removed() {
        let diffs = vec![SyncDiff::Removed {
            type_name: "Person".to_string(),
        }];

        let ontology = HashMap::new();
        let breaking = detect_breaking_changes(&diffs, &ontology);

        assert_eq!(breaking.len(), 1);
        assert!(matches!(breaking[0], BreakingChange::TypeRemoved { .. }));
    }

    #[test]
    fn test_detect_breaking_changes_field_removed() {
        let diffs = vec![SyncDiff::Modified {
            type_name: "Person".to_string(),
            field_changes: vec![FieldChange::Removed {
                name: "email".to_string(),
                field_type: "String".to_string(),
            }],
        }];

        let ontology = HashMap::new();
        let breaking = detect_breaking_changes(&diffs, &ontology);

        assert_eq!(breaking.len(), 1);
        match &breaking[0] {
            BreakingChange::FieldRemoved {
                type_name,
                field_name,
                ..
            } => {
                assert_eq!(type_name, "Person");
                assert_eq!(field_name, "email");
            }
            _ => panic!("Expected FieldRemoved"),
        }
    }

    #[test]
    fn test_detect_breaking_changes_type_changed() {
        let diffs = vec![SyncDiff::Modified {
            type_name: "Person".to_string(),
            field_changes: vec![FieldChange::TypeMismatch {
                name: "age".to_string(),
                ontology_type: "i64".to_string(),
                code_type: "i32".to_string(),
            }],
        }];

        let ontology = HashMap::new();
        let breaking = detect_breaking_changes(&diffs, &ontology);

        assert_eq!(breaking.len(), 1);
        match &breaking[0] {
            BreakingChange::FieldTypeChanged {
                type_name,
                field_name,
                old_type,
                new_type,
            } => {
                assert_eq!(type_name, "Person");
                assert_eq!(field_name, "age");
                assert_eq!(old_type, "i32");
                assert_eq!(new_type, "i64");
            }
            _ => panic!("Expected FieldTypeChanged"),
        }
    }

    #[test]
    fn test_detect_breaking_changes_required_field_added() {
        let diffs = vec![SyncDiff::Modified {
            type_name: "Person".to_string(),
            field_changes: vec![FieldChange::Added {
                name: "email".to_string(),
                field_type: "String".to_string(), // Required (not Option)
            }],
        }];

        let ontology = HashMap::new();
        let breaking = detect_breaking_changes(&diffs, &ontology);

        assert_eq!(breaking.len(), 1);
        assert!(matches!(
            breaking[0],
            BreakingChange::RequiredFieldAdded { .. }
        ));
    }

    #[test]
    fn test_detect_breaking_changes_optional_field_not_breaking() {
        let diffs = vec![SyncDiff::Modified {
            type_name: "Person".to_string(),
            field_changes: vec![FieldChange::Added {
                name: "email".to_string(),
                field_type: "Option<String>".to_string(), // Optional
            }],
        }];

        let ontology = HashMap::new();
        let breaking = detect_breaking_changes(&diffs, &ontology);

        // Optional field added is NOT breaking
        assert_eq!(breaking.len(), 0);
    }

    #[test]
    fn test_generate_migration_drop_table() {
        let changes = vec![BreakingChange::TypeRemoved {
            type_name: "Person".to_string(),
        }];

        let migrations = generate_migrations(&changes, DatabaseBackend::Sqlite);

        assert_eq!(migrations.len(), 1);
        assert!(migrations[0].up_sql.contains("DROP TABLE IF EXISTS person"));
        assert!(migrations[0].description.contains("drop_table_person"));
    }

    #[test]
    fn test_generate_migration_drop_column() {
        let changes = vec![BreakingChange::FieldRemoved {
            type_name: "Person".to_string(),
            field_name: "email".to_string(),
            field_type: "String".to_string(),
        }];

        let migrations = generate_migrations(&changes, DatabaseBackend::Postgres);

        assert_eq!(migrations.len(), 1);
        assert!(
            migrations[0]
                .up_sql
                .contains("ALTER TABLE person DROP COLUMN email")
        );
        assert!(
            migrations[0]
                .down_sql
                .contains("ALTER TABLE person ADD COLUMN email")
        );
    }

    #[test]
    fn test_generate_migration_alter_column() {
        let changes = vec![BreakingChange::FieldTypeChanged {
            type_name: "Person".to_string(),
            field_name: "age".to_string(),
            old_type: "i32".to_string(),
            new_type: "i64".to_string(),
        }];

        let migrations = generate_migrations(&changes, DatabaseBackend::Postgres);

        assert_eq!(migrations.len(), 1);
        assert!(
            migrations[0]
                .up_sql
                .contains("ALTER TABLE person ALTER COLUMN age TYPE BIGINT")
        );
    }

    #[test]
    fn test_migration_filenames() {
        let migration = Migration {
            timestamp: "20260210120000".to_string(),
            description: "add_user_table".to_string(),
            up_sql: "CREATE TABLE users;".to_string(),
            down_sql: "DROP TABLE users;".to_string(),
        };

        assert_eq!(
            migration.up_filename(),
            "20260210120000_add_user_table.up.sql"
        );
        assert_eq!(
            migration.down_filename(),
            "20260210120000_add_user_table.down.sql"
        );
    }
}
