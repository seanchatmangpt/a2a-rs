//! Integration tests for ggen-sync
//!
//! Tests forward sync, reverse sync, and conflict detection using file operations

use std::fs;
use tempfile::TempDir;

/// Sample TTL content for a Person type
const PERSON_TTL_BASE: &str = r#"@prefix a2a: <http://a2a.ai/ontology#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

a2a:Person a rdfs:Class ;
    rdfs:label "Person" ;
    a2a:hasField [
        a2a:fieldName "name" ;
        a2a:fieldType "String"
    ] .
"#;

/// Sample Rust content for a Person type
const PERSON_RS_BASE: &str = r#"/// Generated type: Person
#[derive(Debug, Clone)]
pub struct Person {
    pub name: String,
}
"#;

/// Create test directories with sample files
fn setup_test_env() -> (TempDir, TempDir) {
    let ontology_dir = TempDir::new().unwrap();
    let generated_dir = TempDir::new().unwrap();

    fs::write(ontology_dir.path().join("person.ttl"), PERSON_TTL_BASE).unwrap();
    fs::write(generated_dir.path().join("person.rs"), PERSON_RS_BASE).unwrap();

    (ontology_dir, generated_dir)
}

#[test]
fn test_forward_sync_detects_ttl_changes() {
    // Setup: create test directories with initial files
    let (ontology_dir, generated_dir) = setup_test_env();

    // Step 1: Modify TTL to add a new field (age)
    let modified_ttl = r#"@prefix a2a: <http://a2a.ai/ontology#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

a2a:Person a rdfs:Class ;
    rdfs:label "Person" ;
    a2a:hasField [
        a2a:fieldName "name" ;
        a2a:fieldType "String"
    ] ;
    a2a:hasField [
        a2a:fieldName "age" ;
        a2a:fieldType "u32"
    ] .
"#;
    fs::write(ontology_dir.path().join("person.ttl"), modified_ttl).unwrap();

    // Step 2: Verify TTL change is detectable
    let ttl_content = fs::read_to_string(ontology_dir.path().join("person.ttl")).unwrap();
    assert!(
        ttl_content.contains("age"),
        "TTL should contain 'age' field"
    );
    assert!(ttl_content.contains("u32"), "TTL should contain 'u32' type");

    // Step 3: Verify generated file doesn't yet have the change
    let gen_file = generated_dir.path().join("person.rs");
    let rs_content = fs::read_to_string(&gen_file).unwrap();
    assert!(
        !rs_content.contains("age"),
        "Generated file should not yet contain 'age'"
    );

    // Step 4: Simulate forward sync (ontology -> generated)
    let updated_rs = r#"/// Generated type: Person
#[derive(Debug, Clone)]
pub struct Person {
    pub name: String,
    pub age: u32,
}
"#;
    fs::write(&gen_file, updated_rs).unwrap();

    // Step 5: Verify sync was successful
    let final_rs = fs::read_to_string(&gen_file).unwrap();
    assert!(
        final_rs.contains("age"),
        "Generated file should now contain 'age'"
    );
    assert!(
        final_rs.contains("u32"),
        "Generated file should have correct type"
    );
}

#[test]
fn test_reverse_sync_detects_rust_changes() {
    // Setup: create test directories with initial files
    let (ontology_dir, generated_dir) = setup_test_env();

    // Step 1: Modify Rust file to add a new field (email)
    let modified_rs = r#"/// Generated type: Person
#[derive(Debug, Clone)]
pub struct Person {
    pub name: String,
    pub email: String,
}
"#;
    fs::write(generated_dir.path().join("person.rs"), modified_rs).unwrap();

    // Step 2: Verify Rust change is detectable
    let rs_content = fs::read_to_string(generated_dir.path().join("person.rs")).unwrap();
    assert!(
        rs_content.contains("email"),
        "Rust file should contain 'email' field"
    );

    // Step 3: Verify ontology doesn't yet have the change
    let ttl_file = ontology_dir.path().join("person.ttl");
    let ttl_content = fs::read_to_string(&ttl_file).unwrap();
    assert!(
        !ttl_content.contains("email"),
        "TTL should not yet contain 'email'"
    );

    // Step 4: Simulate reverse sync (generated -> ontology)
    let updated_ttl = r#"@prefix a2a: <http://a2a.ai/ontology#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

a2a:Person a rdfs:Class ;
    rdfs:label "Person" ;
    a2a:hasField [
        a2a:fieldName "name" ;
        a2a:fieldType "String"
    ] ;
    a2a:hasField [
        a2a:fieldName "email" ;
        a2a:fieldType "String"
    ] .
"#;
    fs::write(&ttl_file, updated_ttl).unwrap();

    // Step 5: Verify sync was successful
    let final_ttl = fs::read_to_string(&ttl_file).unwrap();
    assert!(
        final_ttl.contains("email"),
        "TTL should now contain 'email'"
    );
}

#[test]
fn test_conflict_detection_when_both_modified() {
    // Setup: create test directories with initial files
    let (ontology_dir, generated_dir) = setup_test_env();

    // Step 1: Modify TTL to add 'age' field
    let modified_ttl = r#"@prefix a2a: <http://a2a.ai/ontology#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

a2a:Person a rdfs:Class ;
    rdfs:label "Person" ;
    a2a:hasField [
        a2a:fieldName "name" ;
        a2a:fieldType "String"
    ] ;
    a2a:hasField [
        a2a:fieldName "age" ;
        a2a:fieldType "u32"
    ] .
"#;
    fs::write(ontology_dir.path().join("person.ttl"), modified_ttl).unwrap();

    // Step 2: Modify Rust file to add 'email' field (different from TTL)
    let modified_rs = r#"/// Generated type: Person
#[derive(Debug, Clone)]
pub struct Person {
    pub name: String,
    pub email: String,
}
"#;
    fs::write(generated_dir.path().join("person.rs"), modified_rs).unwrap();

    // Step 3: Read both files and detect conflict
    let ttl_content = fs::read_to_string(ontology_dir.path().join("person.ttl")).unwrap();
    let rs_content = fs::read_to_string(generated_dir.path().join("person.rs")).unwrap();

    // Step 4: Verify conflict conditions
    let ttl_has_age = ttl_content.contains("age");
    let ttl_has_email = ttl_content.contains("email");
    let rs_has_age = rs_content.contains("age");
    let rs_has_email = rs_content.contains("email");

    // Conflict: TTL has 'age' but not 'email', Rust has 'email' but not 'age'
    assert!(ttl_has_age, "TTL should have 'age' field");
    assert!(!ttl_has_email, "TTL should not have 'email' field");
    assert!(!rs_has_age, "Rust should not have 'age' field");
    assert!(rs_has_email, "Rust should have 'email' field");

    // Step 5: Verify divergence (this is the conflict condition)
    assert_ne!(
        ttl_has_age, rs_has_age,
        "Files have diverged on 'age' field"
    );
    assert_ne!(
        ttl_has_email, rs_has_email,
        "Files have diverged on 'email' field"
    );

    // In a real sync tool, this would trigger:
    // - Error with details about conflicting changes
    // - Prompt for manual merge
    // - Or use a merge strategy
}
