//! Code generation from RDF ontology via SPARQL CONSTRUCT queries and Tera templates
//!
//! Implements the ggen generate workflow:
//! 1. Parse ggen.toml configuration
//! 2. Load RDF ontology sources into an in-memory store
//! 3. Execute SPARQL CONSTRUCT queries against the ontology
//! 4. Apply Tera templates to CONSTRUCT results
//! 5. Write generated files to output directory

use oxigraph::model::Triple;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tera::{Context, Tera};
use thiserror::Error;

/// Errors that can occur during code generation
#[derive(Debug, Error)]
pub enum GenerateError {
    #[error("Failed to read config file: {0}")]
    ConfigRead(#[from] std::io::Error),

    #[error("Failed to parse TOML config: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("Failed to load ontology: {0}")]
    OntologyLoad(String),

    #[error("Failed to execute SPARQL query: {0}")]
    SparqlExecution(String),

    #[error("Failed to initialize template engine: {0}")]
    TemplateInit(String),

    #[error("Failed to render template: {0}")]
    TemplateRender(#[from] tera::Error),

    #[error("Failed to write output file: {0}")]
    OutputWrite(String),

    #[error("Rule '{0}' is missing required field: {1}")]
    InvalidRule(String, String),
}

/// Top-level ggen.toml configuration
#[derive(Debug, Clone, Deserialize)]
pub struct GgenConfig {
    pub project: ProjectConfig,
    pub ontology: OntologyConfig,
    pub generation: GenerationConfig,
    pub rules: Vec<RuleConfig>,
}

/// Project metadata section
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub description: String,
}

/// Ontology source configuration
#[derive(Debug, Clone, Deserialize)]
pub struct OntologyConfig {
    pub source: PathBuf,
    #[serde(default)]
    pub additional_sources: Vec<PathBuf>,
    pub base_iri: String,
    pub prefixes: HashMap<String, String>,
}

/// Generation output settings
#[derive(Debug, Clone, Deserialize)]
pub struct GenerationConfig {
    pub output_dir: PathBuf,
}

/// A single generation rule (CONSTRUCT query + template)
#[derive(Debug, Clone, Deserialize)]
pub struct RuleConfig {
    pub name: String,
    pub description: String,
    pub template: PathBuf,
    pub output: String,
    pub construct: String,
}

/// Result of code generation
#[derive(Debug, Clone)]
pub struct GenerationResult {
    /// List of files that were generated
    pub generated_files: Vec<PathBuf>,
    /// List of rules that were successfully executed
    pub executed_rules: Vec<String>,
}

/// Load ggen.toml configuration from a file
pub fn load_config(config_path: &Path) -> Result<GgenConfig, GenerateError> {
    let content = fs::read_to_string(config_path)?;
    let config: GgenConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Generate code from ontology using ggen.toml configuration
pub fn generate(config_path: &Path) -> Result<GenerationResult, GenerateError> {
    let config = load_config(config_path)?;
    let config_dir = config_path.parent().ok_or_else(|| {
        GenerateError::ConfigRead(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Config file has no parent directory",
        ))
    })?;

    // Load ontology into RDF store
    let store = load_ontology(&config.ontology, config_dir)?;

    // Initialize Tera template engine
    let tera = init_templates(&config, config_dir)?;

    // Execute each rule and collect results
    let mut generated_files = Vec::new();
    let mut executed_rules = Vec::new();

    for rule in &config.rules {
        let output_path = execute_rule(&store, &tera, rule, &config.generation, config_dir)?;
        generated_files.push(output_path);
        executed_rules.push(rule.name.clone());
    }

    Ok(GenerationResult {
        generated_files,
        executed_rules,
    })
}

/// Load all ontology sources into an Oxigraph store
fn load_ontology(
    ontology_config: &OntologyConfig,
    base_dir: &Path,
) -> Result<Store, GenerateError> {
    let store = Store::new().map_err(|e| GenerateError::OntologyLoad(e.to_string()))?;

    // Load main source
    let main_source = base_dir.join(&ontology_config.source);
    load_turtle_file(&store, &main_source)?;

    // Load additional sources
    for source in &ontology_config.additional_sources {
        let source_path = base_dir.join(source);
        load_turtle_file(&store, &source_path)?;
    }

    Ok(store)
}

/// Load a single Turtle file into the store
fn load_turtle_file(store: &Store, path: &Path) -> Result<(), GenerateError> {
    let content = fs::read_to_string(path).map_err(|e| {
        GenerateError::OntologyLoad(format!("Failed to read {}: {}", path.display(), e))
    })?;

    store
        .load_from_reader(oxigraph::io::RdfFormat::Turtle, content.as_bytes())
        .map_err(|e| {
            GenerateError::OntologyLoad(format!("Failed to parse {}: {}", path.display(), e))
        })?;

    Ok(())
}

/// Initialize Tera template engine
fn init_templates(config: &GgenConfig, base_dir: &Path) -> Result<Tera, GenerateError> {
    let mut tera = Tera::default();

    // Load all templates referenced by rules
    for rule in &config.rules {
        let template_path = base_dir.join(&rule.template);
        let template_content = fs::read_to_string(&template_path).map_err(|e| {
            GenerateError::TemplateInit(format!(
                "Failed to read template {}: {}",
                template_path.display(),
                e
            ))
        })?;

        tera.add_raw_template(&rule.name, &template_content)
            .map_err(|e| {
                GenerateError::TemplateInit(format!(
                    "Failed to add template {}: {}",
                    template_path.display(),
                    e
                ))
            })?;
    }

    Ok(tera)
}

/// Execute a single rule: CONSTRUCT query + template rendering
fn execute_rule(
    store: &Store,
    tera: &Tera,
    rule: &RuleConfig,
    generation: &GenerationConfig,
    base_dir: &Path,
) -> Result<PathBuf, GenerateError> {
    // Execute SPARQL CONSTRUCT query
    let construct_graph = execute_construct(store, &rule.construct, &rule.name)?;

    // Convert graph to template context
    let context = graph_to_context(construct_graph)?;

    // Render template
    let rendered = tera.render(&rule.name, &context)?;

    // Write output file
    let output_path = base_dir.join(&generation.output_dir).join(&rule.output);

    // Create parent directories if needed
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            GenerateError::OutputWrite(format!(
                "Failed to create directory {}: {}",
                parent.display(),
                e
            ))
        })?;
    }

    fs::write(&output_path, rendered).map_err(|e| {
        GenerateError::OutputWrite(format!("Failed to write {}: {}", output_path.display(), e))
    })?;

    Ok(output_path)
}

/// Execute a SPARQL CONSTRUCT query and return the resulting graph
fn execute_construct(
    store: &Store,
    query: &str,
    rule_name: &str,
) -> Result<Vec<Triple>, GenerateError> {
    let results = store.query(query).map_err(|e| {
        GenerateError::SparqlExecution(format!("Query failed for rule '{}': {}", rule_name, e))
    })?;

    match results {
        QueryResults::Graph(graph) => {
            let triples: Vec<_> = graph.collect::<Result<Vec<_>, _>>().map_err(|e| {
                GenerateError::SparqlExecution(format!(
                    "Failed to collect graph for rule '{}': {}",
                    rule_name, e
                ))
            })?;
            Ok(triples)
        }
        _ => Err(GenerateError::SparqlExecution(format!(
            "Rule '{}' did not return a graph (expected CONSTRUCT query)",
            rule_name
        ))),
    }
}

/// Convert RDF graph from CONSTRUCT to Tera template context
fn graph_to_context(triples: Vec<Triple>) -> Result<Context, GenerateError> {
    let mut context = Context::new();

    // Group triples by subject to build structured data
    let mut subjects: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();

    for triple in triples {
        let subject = triple.subject.to_string();
        let predicate = triple.predicate.to_string();
        let object = format_object(&triple.object);

        subjects
            .entry(subject)
            .or_default()
            .entry(predicate)
            .or_default()
            .push(object);
    }

    // Convert to a more template-friendly structure
    // This is a simplified version - real implementation would need to
    // interpret the GeneratedStruct/StructField/etc. predicates
    context.insert("triples", &subjects);

    Ok(context)
}

/// Format an RDF term as a string for template context
fn format_object(term: &oxigraph::model::Term) -> String {
    match term {
        oxigraph::model::Term::NamedNode(n) => n.to_string(),
        oxigraph::model::Term::BlankNode(b) => b.to_string(),
        oxigraph::model::Term::Literal(l) => l.value().to_string(),
        oxigraph::model::Term::Triple(t) => {
            format!("<< {} {} {} >>", t.subject, t.predicate, t.object)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("ggen.toml");

        let config_content = r#"
[project]
name = "test-project"
version = "0.1.0"
description = "Test project"

[ontology]
source = "ontology/main.ttl"
additional_sources = ["ontology/extra.ttl"]
base_iri = "https://example.org/ontology/"

[ontology.prefixes]
ex = "https://example.org/ontology/"
rdf = "http://www.w3.org/1999/02/22-rdf-syntax-ns#"

[generation]
output_dir = "src/generated"

[[rules]]
name = "test-rule"
description = "Test rule"
template = "templates/test.tera"
output = "test.rs"
construct = "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }"
"#;

        fs::write(&config_path, config_content).unwrap();

        let config = load_config(&config_path).unwrap();
        assert_eq!(config.project.name, "test-project");
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].name, "test-rule");
    }

    #[test]
    fn test_load_turtle_file() {
        let temp_dir = TempDir::new().unwrap();
        let ttl_path = temp_dir.path().join("test.ttl");

        let ttl_content = r#"
@prefix ex: <https://example.org/> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

ex:Subject1 rdf:type ex:TestClass .
ex:Subject1 ex:name "Test Name" .
"#;

        fs::write(&ttl_path, ttl_content).unwrap();

        let store = Store::new().unwrap();
        load_turtle_file(&store, &ttl_path).unwrap();

        // Verify the store has triples
        let count = store.iter().count();
        assert!(count > 0, "Store should contain triples");
    }

    #[test]
    fn test_execute_construct_query() {
        let store = Store::new().unwrap();

        // Add some test data using update query
        let update = r#"
PREFIX ex: <https://example.org/>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

INSERT DATA {
    ex:Subject1 rdf:type ex:TestClass .
}
"#;

        store.update(update).unwrap();

        // Execute CONSTRUCT query
        let query = r#"
PREFIX ex: <https://example.org/>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

CONSTRUCT { ?s rdf:type ?o }
WHERE { ?s rdf:type ?o }
"#;

        let result = execute_construct(&store, query, "test-rule").unwrap();
        assert!(!result.is_empty(), "CONSTRUCT should return triples");
    }
}
