# ggen-sync Web Viewer

Interactive web-based viewer for visualizing differences between RDF ontology and generated Rust code.

## Features

- Side-by-side comparison of ontology vs code
- Color-coded diff types:
  - **Green (Added)**: Type exists in ontology but not in code
  - **Yellow (Modified)**: Type exists in both with field differences
  - **Red (Removed)**: Type exists in code but not in ontology
- Field-level change details for modified types
- Summary statistics
- Multiple data loading methods:
  - Fetch from API endpoint
  - Upload JSON file
  - Load sample data

## Usage

### Method 1: Load from JSON File

1. Generate a JSON report:
   ```bash
   cd ggen-sync
   cargo run --example json_reporter
   ```

2. Open `web/index.html` in your browser

3. Click the file input and select `sync-status.json`

### Method 2: Serve with API Endpoint

If you have a server providing sync data at `/api/sync-status`, simply:

1. Open `web/index.html` in your browser
2. Click "Fetch from API"

The viewer expects JSON in the format produced by `ggen_sync::report_sync_json()`:

```json
[
  {
    "Added": {
      "type_name": "NewType"
    }
  },
  {
    "Modified": {
      "type_name": "ExistingType",
      "field_changes": [
        {
          "Added": {
            "name": "new_field",
            "field_type": "String"
          }
        }
      ]
    }
  },
  {
    "Removed": {
      "type_name": "OldType"
    }
  }
]
```

### Method 3: Try Sample Data

Click "Load Sample Data" to see the viewer with example diffs.

## Integration

To integrate with your sync detection workflow:

```rust
use ggen_sync::{detect_diffs, read_ontology, read_generated_code, report_sync_json};

// Detect diffs
let ontology = read_ontology("path/to/ontology")?;
let code = read_generated_code("path/to/code")?;
let diffs = detect_diffs(&ontology, &code);

// Generate JSON for web viewer
let json = report_sync_json(&diffs)?;
println!("{}", json);
```

## Pure JavaScript

No frameworks, no build step. Just open `index.html` in any modern browser.
