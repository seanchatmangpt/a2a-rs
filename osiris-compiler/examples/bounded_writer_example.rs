//! Example demonstrating CONSTRUCT8 bounded writer usage.
//!
//! This example shows:
//! 1. Creating patches with bounded mutations (≤8 units)
//! 2. Validating patches before commit
//! 3. Atomic commits with the InMemoryWriter
//! 4. Handling validation errors

use osiris_compiler::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the writer
    let writer = InMemoryWriter::new();

    println!("=== CONSTRUCT8 Bounded Writer Example ===\n");
    println!(
        "Maximum mutation units per commit: {}\n",
        writer.max_mutation_units()
    );

    // Example 1: Valid patch within limits
    println!("1. Creating a valid patch with 3 triples...");
    let mut patch1 = Patch::new();
    patch1.add(Triple::new(
        "http://example.org/user/alice",
        "http://xmlns.com/foaf/0.1/name",
        "Alice Smith",
    ));
    patch1.add(Triple::new(
        "http://example.org/user/alice",
        "http://xmlns.com/foaf/0.1/email",
        "alice@example.org",
    ));
    patch1.add(Triple::new(
        "http://example.org/user/alice",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
        "http://xmlns.com/foaf/0.1/Person",
    ));

    match writer.validate_patch(&patch1).await {
        Ok(_) => println!("   ✓ Patch validated successfully"),
        Err(e) => println!("   ✗ Validation failed: {}", e),
    }

    let result1 = writer.commit_patch(patch1).await?;
    println!(
        "   ✓ Committed {} additions, {} deletions",
        result1.additions_count, result1.deletions_count
    );
    println!("   Commit ID: {}", result1.patch_set_id);
    println!("   Timestamp: {}\n", result1.timestamp);

    // Example 2: Patch at the limit (8 mutations)
    println!("2. Creating a patch at the limit (8 triples)...");
    let mut patch2 = Patch::new();
    for i in 1..=8 {
        patch2.add(Triple::new(
            format!("http://example.org/item/{}", i),
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://example.org/Item",
        ));
    }

    match writer.validate_patch(&patch2).await {
        Ok(_) => println!("   ✓ Patch at limit validated successfully"),
        Err(e) => println!("   ✗ Validation failed: {}", e),
    }

    let result2 = writer.commit_patch(patch2).await?;
    println!("   ✓ Committed {} additions\n", result2.additions_count);

    // Example 3: Invalid patch exceeding limits
    println!("3. Creating an invalid patch (9 triples, exceeds limit)...");
    let mut patch3 = Patch::new();
    for i in 1..=9 {
        patch3.add(Triple::new(
            format!("http://example.org/item/{}", i),
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://example.org/Item",
        ));
    }

    match writer.validate_patch(&patch3).await {
        Ok(_) => println!("   ✓ Patch validated (unexpected!)"),
        Err(WriteError::ValidationFailed(PatchError::ExceedsLimit { actual, max })) => {
            println!(
                "   ✓ Validation correctly rejected patch: {} > {}",
                actual, max
            );
        }
        Err(e) => println!("   ✗ Unexpected error: {}", e),
    }

    match writer.commit_patch(patch3).await {
        Ok(_) => println!("   ✗ Commit succeeded (should have failed!)"),
        Err(WriteError::ValidationFailed(PatchError::ExceedsLimit { actual, max })) => {
            println!("   ✓ Commit correctly rejected: {} > {}\n", actual, max);
        }
        Err(e) => println!("   ✗ Unexpected error: {}\n", e),
    }

    // Example 4: Patch with additions and deletions
    println!("4. Creating a patch with both additions and deletions...");
    let mut patch4 = Patch::new();
    // Delete one triple
    patch4.delete(Triple::new(
        "http://example.org/user/alice",
        "http://xmlns.com/foaf/0.1/email",
        "alice@example.org",
    ));
    // Add updated triple
    patch4.add(Triple::new(
        "http://example.org/user/alice",
        "http://xmlns.com/foaf/0.1/email",
        "alice.smith@example.org",
    ));

    println!("   Mutation count: {}", patch4.mutation_count());
    let result4 = writer.commit_patch(patch4).await?;
    println!(
        "   ✓ Committed {} additions, {} deletions\n",
        result4.additions_count, result4.deletions_count
    );

    // Example 5: Patch set (atomic multi-patch commit)
    println!("5. Creating a patch set with 2 patches...");
    let mut p1 = Patch::new();
    p1.add(Triple::new(
        "http://example.org/user/bob",
        "http://xmlns.com/foaf/0.1/name",
        "Bob Jones",
    ));

    let mut p2 = Patch::new();
    p2.add(Triple::new(
        "http://example.org/user/bob",
        "http://xmlns.com/foaf/0.1/knows",
        "http://example.org/user/alice",
    ));

    let patch_set = PatchSet::new(vec![p1, p2]);
    println!(
        "   Total mutations in set: {}",
        patch_set.total_mutation_count()
    );

    let result5 = writer.commit_patch_set(patch_set).await?;
    println!(
        "   ✓ Committed patch set with {} total mutations\n",
        result5.additions_count + result5.deletions_count
    );

    // Summary
    println!("=== Summary ===");
    println!("Total triples in store: {}", writer.triple_count());
    println!("Total commits: {}", writer.commit_history().len());

    Ok(())
}
