# Quick Start Guide

Get the web viewer running in 2 minutes.

## Step 1: Generate Sample Data

```bash
cd ggen-sync
cargo run --example json_reporter
```

This creates `sync-status.json` with sample sync differences.

## Step 2: View the Data

### Option A: File Upload (Simplest)

1. Open `web/index.html` in your browser
2. Click the file input
3. Select `sync-status.json`
4. View the side-by-side comparison

### Option B: HTTP Server (Full Experience)

```bash
cargo run --example web_server
```

Then open http://localhost:8080 and click "Fetch from API".

### Option C: Sample Data (Demo)

1. Open `web/index.html` in your browser
2. Click "Load Sample Data"
3. See example diffs immediately

## Understanding the Output

### Added (Green)
Type exists in ontology but not in generated code.
- **Action**: Generate the missing type from ontology

### Modified (Yellow)
Type exists in both but has field differences.
- **Field Added**: In ontology, not in code - regenerate
- **Field Removed**: In code, not in ontology - remove from code or add to ontology
- **Type Mismatch**: Field type differs - choose correct type and sync

### Removed (Red)
Type exists in code but not in ontology.
- **Action**: Remove from code or add to ontology

## Next Steps

- Generate real diffs from your ontology and code
- Use the viewer to identify sync issues
- Apply forward or reverse sync as needed
- Verify changes in the viewer

## Troubleshooting

**Problem**: File upload doesn't work
- **Solution**: Use the HTTP server or check browser console for errors

**Problem**: API fetch fails
- **Solution**: Make sure web_server is running on port 8080

**Problem**: JSON parse error
- **Solution**: Verify JSON file format matches the expected structure
