//! Demonstration of ggen generate functionality
//!
//! This example shows how to use the generate module to:
//! 1. Load ggen.toml configuration
//! 2. Execute SPARQL CONSTRUCT queries against RDF ontology
//! 3. Apply Tera templates to generate Rust code

use ggen_sync::load_config;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Path to ggen.toml configuration
    let config_path = Path::new("ggen/ggen.toml");

    if !config_path.exists() {
        eprintln!("Error: ggen.toml not found at {}", config_path.display());
        eprintln!("This example expects to be run from the workspace root");
        return Ok(());
    }

    println!("Loading configuration from {}", config_path.display());
    let config = load_config(config_path)?;

    println!("\n=== Configuration ===");
    println!(
        "Project: {} v{}",
        config.project.name, config.project.version
    );
    println!("Description: {}", config.project.description);
    println!(
        "Output directory: {}",
        config.generation.output_dir.display()
    );
    println!("Number of rules: {}", config.rules.len());

    println!("\n=== Ontology Sources ===");
    println!("Main source: {}", config.ontology.source.display());
    for source in &config.ontology.additional_sources {
        println!("Additional: {}", source.display());
    }

    println!("\n=== Generation Rules ===");
    for rule in &config.rules {
        println!("\n[{}]", rule.name);
        println!("  Description: {}", rule.description);
        println!("  Template: {}", rule.template.display());
        println!("  Output: {}", rule.output);
        println!("  CONSTRUCT query length: {} chars", rule.construct.len());
    }

    // Note: Actual generation is commented out to avoid modifying generated files
    // Uncomment to run full generation:
    //
    // println!("\n=== Generating Code ===");
    // let result = generate(config_path)?;
    //
    // println!("\nGeneration complete!");
    // println!("Executed {} rules", result.executed_rules.len());
    // println!("\nGenerated files:");
    // for file in &result.generated_files {
    //     println!("  - {}", file.display());
    // }

    println!("\n=== Demo Complete ===");
    println!("To actually generate code, uncomment the generation section in this example");

    Ok(())
}
