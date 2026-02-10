//! Conflict resolution for ontology-code synchronization
//!
//! When both ontology and code changed the same type, detect conflicts
//! and resolve them either interactively or automatically.

use crate::types::{FieldChange, SyncDiff};
use std::io::{self, Write};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResolverError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid choice: {0}")]
    InvalidChoice(String),

    #[error("Resolution cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, ResolverError>;

/// Resolution strategy for conflicts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStrategy {
    /// Take ontology version (forward sync)
    TakeOntology,
    /// Take code version (reverse sync)
    TakeCode,
    /// Merge both versions (manual)
    Merge,
}

/// Configuration for conflict resolution
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    /// Use automatic resolution (no prompts)
    pub auto: bool,
    /// Default strategy for auto mode
    pub default_strategy: ResolutionStrategy,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            auto: false,
            default_strategy: ResolutionStrategy::TakeOntology,
        }
    }
}

impl ResolverConfig {
    /// Create config for interactive mode
    pub fn interactive() -> Self {
        Self {
            auto: false,
            default_strategy: ResolutionStrategy::TakeOntology,
        }
    }

    /// Create config for auto mode with specified strategy
    pub fn auto(strategy: ResolutionStrategy) -> Self {
        Self {
            auto: true,
            default_strategy: strategy,
        }
    }
}

/// Action to take after resolving a conflict
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncAction {
    /// Generate code from ontology
    GenerateCode { type_name: String },
    /// Update ontology from code
    UpdateOntology { type_name: String },
    /// Update existing code with ontology changes
    UpdateCode { type_name: String },
    /// Skip this conflict
    Skip { type_name: String },
}

/// Resolve conflicts in sync diffs
///
/// Returns a list of actions to take for each conflict.
/// In auto mode, uses the configured strategy for all conflicts.
/// In interactive mode, prompts the user for each conflict.
pub fn resolve_conflicts(diffs: Vec<SyncDiff>, config: &ResolverConfig) -> Result<Vec<SyncAction>> {
    let mut actions = Vec::new();

    for diff in diffs {
        let action = if config.auto {
            // Auto mode: use default strategy
            resolve_auto(&diff, config.default_strategy)
        } else {
            // Interactive mode: prompt user
            resolve_interactive(&diff)?
        };

        if let Some(action) = action {
            actions.push(action);
        }
    }

    Ok(actions)
}

/// Resolve a diff automatically based on strategy
fn resolve_auto(diff: &SyncDiff, strategy: ResolutionStrategy) -> Option<SyncAction> {
    match (diff, strategy) {
        // TakeOntology: ontology wins
        (SyncDiff::Added { type_name }, ResolutionStrategy::TakeOntology) => {
            Some(SyncAction::GenerateCode {
                type_name: type_name.clone(),
            })
        }
        (SyncDiff::Removed { type_name }, ResolutionStrategy::TakeOntology) => {
            // Type removed from ontology - skip for safety
            Some(SyncAction::Skip {
                type_name: type_name.clone(),
            })
        }
        (SyncDiff::Modified { type_name, .. }, ResolutionStrategy::TakeOntology) => {
            Some(SyncAction::UpdateCode {
                type_name: type_name.clone(),
            })
        }

        // TakeCode: code wins
        (SyncDiff::Added { type_name }, ResolutionStrategy::TakeCode) => {
            // Type in ontology but not in code - skip
            Some(SyncAction::Skip {
                type_name: type_name.clone(),
            })
        }
        (SyncDiff::Removed { type_name }, ResolutionStrategy::TakeCode) => {
            Some(SyncAction::UpdateOntology {
                type_name: type_name.clone(),
            })
        }
        (SyncDiff::Modified { type_name, .. }, ResolutionStrategy::TakeCode) => {
            Some(SyncAction::UpdateOntology {
                type_name: type_name.clone(),
            })
        }

        // Merge: not implemented in auto mode
        (diff, ResolutionStrategy::Merge) => {
            let type_name = match diff {
                SyncDiff::Added { type_name }
                | SyncDiff::Removed { type_name }
                | SyncDiff::Modified { type_name, .. } => type_name.clone(),
            };
            eprintln!(
                "Warning: Merge not supported in auto mode, skipping {}",
                type_name
            );
            Some(SyncAction::Skip { type_name })
        }
    }
}

/// Resolve a diff interactively by prompting the user
fn resolve_interactive(diff: &SyncDiff) -> Result<Option<SyncAction>> {
    match diff {
        SyncDiff::Added { type_name } => prompt_for_added(type_name),
        SyncDiff::Removed { type_name } => prompt_for_removed(type_name),
        SyncDiff::Modified {
            type_name,
            field_changes,
        } => prompt_for_modified(type_name, field_changes),
    }
}

/// Prompt user for an added type
fn prompt_for_added(type_name: &str) -> Result<Option<SyncAction>> {
    println!(
        "\n[ADDED] Type '{}' exists in ontology but not in code",
        type_name
    );
    println!("  1) Take ontology (generate code)");
    println!("  2) Take code (skip)");
    println!("  3) Skip this conflict");

    let choice = read_choice()?;
    match choice {
        1 => Ok(Some(SyncAction::GenerateCode {
            type_name: type_name.to_string(),
        })),
        2 | 3 => Ok(Some(SyncAction::Skip {
            type_name: type_name.to_string(),
        })),
        _ => Err(ResolverError::InvalidChoice(choice.to_string())),
    }
}

/// Prompt user for a removed type
fn prompt_for_removed(type_name: &str) -> Result<Option<SyncAction>> {
    println!(
        "\n[REMOVED] Type '{}' exists in code but not in ontology",
        type_name
    );
    println!("  1) Take ontology (remove from code - manual)");
    println!("  2) Take code (update ontology)");
    println!("  3) Skip this conflict");

    let choice = read_choice()?;
    match choice {
        1 => Ok(Some(SyncAction::Skip {
            type_name: type_name.to_string(),
        })),
        2 => Ok(Some(SyncAction::UpdateOntology {
            type_name: type_name.to_string(),
        })),
        3 => Ok(Some(SyncAction::Skip {
            type_name: type_name.to_string(),
        })),
        _ => Err(ResolverError::InvalidChoice(choice.to_string())),
    }
}

/// Prompt user for a modified type (real conflict)
fn prompt_for_modified(
    type_name: &str,
    field_changes: &[FieldChange],
) -> Result<Option<SyncAction>> {
    println!(
        "\n[CONFLICT] Type '{}' modified in both ontology and code",
        type_name
    );
    println!("Changes:");

    for change in field_changes {
        match change {
            FieldChange::Added { name, field_type } => {
                println!("  + {}: {} (in ontology, not in code)", name, field_type);
            }
            FieldChange::Removed { name, field_type } => {
                println!("  - {}: {} (in code, not in ontology)", name, field_type);
            }
            FieldChange::TypeMismatch {
                name,
                ontology_type,
                code_type,
            } => {
                println!(
                    "  ! {}: {} (ontology) vs {} (code)",
                    name, ontology_type, code_type
                );
            }
        }
    }

    println!("\nHow to resolve?");
    println!("  1) Take ontology (update code with ontology changes)");
    println!("  2) Take code (update ontology with code changes)");
    println!("  3) Merge (not yet implemented - will skip)");
    println!("  4) Skip this conflict");

    let choice = read_choice()?;
    match choice {
        1 => Ok(Some(SyncAction::UpdateCode {
            type_name: type_name.to_string(),
        })),
        2 => Ok(Some(SyncAction::UpdateOntology {
            type_name: type_name.to_string(),
        })),
        3 => {
            eprintln!("Merge not yet implemented, skipping");
            Ok(Some(SyncAction::Skip {
                type_name: type_name.to_string(),
            }))
        }
        4 => Ok(Some(SyncAction::Skip {
            type_name: type_name.to_string(),
        })),
        _ => Err(ResolverError::InvalidChoice(choice.to_string())),
    }
}

/// Read a numeric choice from stdin
fn read_choice() -> Result<u32> {
    print!("Enter choice: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    input
        .trim()
        .parse()
        .map_err(|_| ResolverError::InvalidChoice(input.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_mode_take_ontology() {
        let diffs = vec![
            SyncDiff::Added {
                type_name: "NewType".to_string(),
            },
            SyncDiff::Modified {
                type_name: "ChangedType".to_string(),
                field_changes: vec![FieldChange::Added {
                    name: "new_field".to_string(),
                    field_type: "String".to_string(),
                }],
            },
        ];

        let config = ResolverConfig::auto(ResolutionStrategy::TakeOntology);
        let actions = resolve_conflicts(diffs, &config).unwrap();

        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], SyncAction::GenerateCode { .. }));
        assert!(matches!(actions[1], SyncAction::UpdateCode { .. }));
    }

    #[test]
    fn test_auto_mode_take_code() {
        let diffs = vec![
            SyncDiff::Removed {
                type_name: "OldType".to_string(),
            },
            SyncDiff::Modified {
                type_name: "ChangedType".to_string(),
                field_changes: vec![FieldChange::Removed {
                    name: "old_field".to_string(),
                    field_type: "i32".to_string(),
                }],
            },
        ];

        let config = ResolverConfig::auto(ResolutionStrategy::TakeCode);
        let actions = resolve_conflicts(diffs, &config).unwrap();

        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], SyncAction::UpdateOntology { .. }));
        assert!(matches!(actions[1], SyncAction::UpdateOntology { .. }));
    }

    #[test]
    fn test_auto_mode_merge_not_implemented() {
        let diffs = vec![SyncDiff::Modified {
            type_name: "ConflictType".to_string(),
            field_changes: vec![FieldChange::TypeMismatch {
                name: "field".to_string(),
                ontology_type: "String".to_string(),
                code_type: "i32".to_string(),
            }],
        }];

        let config = ResolverConfig::auto(ResolutionStrategy::Merge);
        let actions = resolve_conflicts(diffs, &config).unwrap();

        // Merge falls back to Skip in auto mode
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], SyncAction::Skip { .. }));
    }

    #[test]
    fn test_resolve_auto_added_ontology() {
        let diff = SyncDiff::Added {
            type_name: "NewType".to_string(),
        };
        let action = resolve_auto(&diff, ResolutionStrategy::TakeOntology);
        assert!(matches!(action, Some(SyncAction::GenerateCode { .. })));
    }

    #[test]
    fn test_resolve_auto_removed_code() {
        let diff = SyncDiff::Removed {
            type_name: "OldType".to_string(),
        };
        let action = resolve_auto(&diff, ResolutionStrategy::TakeCode);
        assert!(matches!(action, Some(SyncAction::UpdateOntology { .. })));
    }

    #[test]
    fn test_resolve_auto_modified_ontology() {
        let diff = SyncDiff::Modified {
            type_name: "ChangedType".to_string(),
            field_changes: vec![],
        };
        let action = resolve_auto(&diff, ResolutionStrategy::TakeOntology);
        assert!(matches!(action, Some(SyncAction::UpdateCode { .. })));
    }
}
