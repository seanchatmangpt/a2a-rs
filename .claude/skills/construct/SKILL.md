---
name: construct
description: Generate Rust code from A2A RDF ontology using ggen CONSTRUCT queries
disable-model-invocation: true
allowed-tools: Bash(ggen *), Bash(cargo *), Read, Glob
argument-hint: "[rule-name-or-pattern]"
---

Generate Rust code from the A2A Protocol RDF ontology by running ggen CONSTRUCT queries.

## Available ontology files
!`ls ggen/ontology/*.ttl`

## Current ggen config
!`cat ggen/ggen.toml | head -20`

## Steps

### Step 1: Run ggen

If $ARGUMENTS is provided, run ggen for that specific rule subset:

```
ggen --manifest ggen/ggen.toml --rule "$ARGUMENTS"
```

If no arguments are provided, run ggen with the full manifest:

```
ggen --manifest ggen/ggen.toml
```

### Step 2: Verify generated code compiles

```
cargo check --all-features
```

If compilation fails, report the errors with file paths and line numbers. Do not modify generated files to fix compilation -- the CONSTRUCT queries or ontology need to be corrected upstream.

### Step 3: Diff generated vs hand-written code

Identify the output files that ggen produced (from ggen.toml output paths). For each generated file, check whether a hand-written equivalent already exists in the source tree:

- Use `diff` to compare generated output against any existing hand-written files
- Flag fields, types, or trait signatures that differ
- Note any hand-written additions not present in the generated output (these may be intentional extensions beyond the ontology)

### Step 4: Report

Summarize what was generated:

| Rule | Output File | Status | Notes |
|------|-------------|--------|-------|

Status values:
- **generated** -- new file created, compiles cleanly
- **updated** -- existing file overwritten, compiles cleanly
- **conflict** -- generated output differs from hand-written code (list discrepancies)
- **error** -- ggen or compilation failed (include error message)
