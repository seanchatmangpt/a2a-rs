# ggen-sync

Ontology-code synchronization tool for the ggen ecosystem. Detects and resolves drift between RDF ontology definitions and generated Rust code.

## Features

- **Drift Detection**: Compare RDF ontology with generated Rust code
- **Bidirectional Sync**: Forward (ontology → code) and reverse (code → ontology)
- **Conflict Resolution**: Configurable strategies for handling conflicts
- **Web Viewer**: Interactive HTML viewer for visualizing sync differences
- **JSON Export**: Machine-readable sync reports

## Usage

### Detect Drift

```rust
use ggen_sync::{read_ontology, read_generated_code, detect_diffs, report_sync_status};

let ontology = read_ontology("ggen/ontology")?;
let code = read_generated_code("a2a-rs/src/generated")?;
let diffs = detect_diffs(&ontology, &code);

report_sync_status(&diffs);
```

### Sync Operations

```rust
use ggen_sync::{sync, SyncDirection};

// Forward sync: ontology → code
sync("ggen/ontology", "a2a-rs/src/generated", SyncDirection::Forward)?;

// Reverse sync: code → ontology
sync("ggen/ontology", "a2a-rs/src/generated", SyncDirection::Reverse)?;
```

### JSON Export for Tools

```rust
use ggen_sync::{report_sync_json, write_sync_json};

// Generate JSON string
let json = report_sync_json(&diffs)?;
println!("{}", json);

// Write to file
let mut file = std::fs::File::create("sync-status.json")?;
write_sync_json(&diffs, &mut file)?;
```

## Web Viewer

Interactive visualization of sync differences:

```bash
# Generate sample JSON
cargo run --example json_reporter

# Start web server
cargo run --example web_server

# Open http://localhost:8080 in your browser
```

See [web/README.md](web/README.md) for details.

## Examples

- `json_reporter` - Generate JSON output for the web viewer
- `reporter_demo` - Console output of sync differences
- `resolver_demo` - Conflict resolution strategies
- `web_server` - Serve the web viewer with live data

## Architecture

- `ontology_reader` - Parse RDF/Turtle ontology files
- `code_reader` - Parse generated Rust code
- `differ` - Detect differences between ontology and code
- `forward_sync` - Generate Rust code from ontology
- `reverse_sync` - Generate RDF from Rust code
- `resolver` - Resolve sync conflicts
- `reporter` - Generate human and machine-readable reports
- `web/` - HTML/JavaScript viewer for sync diffs

## Integration

Part of the ggen code generation pipeline:

1. Define domain types in RDF ontology (`ggen/ontology/*.ttl`)
2. Generate Rust code via CONSTRUCT queries
3. Detect drift with ggen-sync
4. Resolve conflicts or regenerate as needed
