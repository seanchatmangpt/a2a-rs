# TPS Mapping: Manufacturing Principles in Code Generation

**Author:** Sean Chatman, Claude Opus 4.6
**Date:** February 9, 2026
**See also:** [PARADIGM-SHIFT.md](PARADIGM-SHIFT.md), [CONSTRUCT.md](../CONSTRUCT.md)

---

## Overview

This document provides a detailed mapping between **Toyota Production System (TPS)** principles and their application in **a2a-rs code generation**. TPS is the foundational manufacturing philosophy that enabled Toyota to achieve zero-defect production with minimal inventory and maximum throughput. We apply the same principles to code generation to achieve zero-defect generated code with minimal work-in-progress and maximum compilation throughput.

---

## The Two Pillars of TPS

### Pillar 1: Just-In-Time (JIT)

**Definition:** Produce only what is needed, when it is needed, in the amount needed.

**Traditional manufacturing problem:** Overproduction creates inventory. Inventory costs money (storage, handling, obsolescence) and hides defects.

**Code generation mapping:**
- **Need:** A type definition for `Message` in Rust
- **When needed:** At compile time
- **Amount:** Exactly one `Message` struct with exactly the fields defined in the ontology

**Anti-pattern (overproduction):**
- Generate `Message`, `MessageBuilder`, `MessageValidator`, `MessageSerializer`, `MessageDeserializer` when only `Message` is used
- Generate types for all protocols when only A2A is needed (feature flag violation)
- Generate documentation, tests, examples that are never consumed

**JIT in a2a-rs:**
- Feature flags (`http-server`, `ws-client`, etc.) enable conditional compilation — only generate what's needed
- CONSTRUCT queries are on-demand — templates only render when invoked
- No speculative code — every line of generated code is used by the library

### Pillar 2: Jidoka (Autonomation)

**Definition:** Build quality into the process. Detect abnormalities and stop the line immediately.

**Traditional manufacturing problem:** Defects that escape to the next station compound. A bad part becomes 10 bad assemblies becomes 100 bad products.

**Code generation mapping:**
- **Abnormality:** Invalid ontology triple, missing field, undefined type
- **Detection:** SHACL validation in μ₁, SPARQL query failure in μ₂, template rendering error in μ₃, compilation failure in μ₄
- **Stop the line:** Pipeline exits with error code 1; no files are written; user sees exact failure reason

**Jidoka in a2a-rs:**
```bash
# μ₁: Normalize - SHACL validation
shacl validate ggen/ontology/a2a-message.ttl ggen/shapes.ttl
# Exit code 1 → ontology is invalid → STOP

# μ₂: Extract - CONSTRUCT query
sparql construct --query structs ggen/ontology/*.ttl
# No results → query pattern didn't match → STOP

# μ₃: Emit - Tera template rendering
tera render ir/structs.ttl templates/struct.tera
# Template error → undefined variable, syntax error → STOP

# μ₄: Canonicalize - Compilation
cargo check --manifest-path a2a-rs/Cargo.toml
# Compile error → generated code is invalid → STOP
```

**No defect escapes.** If any stage fails, the entire pipeline stops and emits a diagnostic receipt.

---

## The TPS Method Stack

### 1. Heijunka (Level Loading)

**Definition:** Even out the production schedule to avoid peaks and valleys. Batch sizes should be small and uniform.

**Traditional manufacturing:** 1000 units on Monday, 0 units Tuesday-Thursday, 500 units Friday → machinery idle 60% of the time, workers stressed on Monday/Friday.

**Code generation mapping:**
- **Uneven loading (SCM):** PRs pile up for a week, then reviewed in batch on Friday → reviewers overwhelmed, authors context-switched away
- **Level loading (CCM):** CONSTRUCT queries run in parallel; every ontology change takes the same amount of time (~1 second); no batching required

**Heijunka in a2a-rs:**
- 7 CONSTRUCT queries are tensor product `Q₁ ⊗ Q₂ ⊗ ... ⊗ Q₇`
- Each query processes its own ontology subgraph
- All queries run concurrently (7-core parallelism)
- Rendering happens in parallel (7 Tera processes)
- Result: Uniform ~1 second per generation, regardless of change size

**Metric:** Standard deviation of generation time. Target: < 0.1 seconds (99% of runs within ±10% of mean).

### 2. Kanban (Pull System)

**Definition:** Downstream processes pull from upstream based on actual demand. No work is pushed until requested.

**Traditional manufacturing:** Upstream factory produces parts; downstream factory consumes them when ready. Kanban card signals "I need 10 more parts."

**Code generation mapping:**
- **Push (SCM):** Manager assigns task to developer; developer must start immediately (even if blocked on other tasks)
- **Pull (CCM):** Agent queries task queue; pulls task only when ready to process; no task is forced

**Kanban in a2a-rs:**
- Skills are kanban cards: `/construct`, `/spec-check`, `/ontology`, `/trace-issue`
- Skills load on-demand, not eagerly
- Agent pulls skill when needed: `Skill("construct")`
- No skill is loaded until invoked → context window is not polluted with unused instructions

**Example:**
```
User: "Add a field to Message"
Assistant:
  1. Edits ggen/ontology/a2a-message.ttl
  2. Invokes Skill("construct") → pulls construct skill → runs generation
  3. Skill completes → context window released
  4. No other skills loaded (no spec-check until user asks)
```

**Metric:** Skill load latency. Target: 0 (skills are invoked exactly when needed, never preemptively).

### 3. Standard Work

**Definition:** Document the best-known method for performing work. All workers follow the same procedure.

**Traditional manufacturing:** Every worker assembles the same part using the same tools in the same sequence. Variation is eliminated.

**Code generation mapping:**
- **Variation (SCM):** Developer A uses `serde_json`, Developer B uses `serde_yaml`, Developer C uses custom serializer → inconsistent output
- **Standard Work (CCM):** All Rust structs generated from the same templates; all serialization uses `#[derive(Serialize, Deserialize)]`; all field naming uses camelCase (via `#[serde(rename_all = "camelCase")]`)

**Standard Work in a2a-rs:**
```tera
{# templates/struct.tera - this is the standard work instruction #}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {{ struct.structName }} {
    {% for field in struct.hasField | sort(attribute="fieldOrder") %}
    pub {{ field.fieldName }}: {{ field.fieldType }},
    {% endfor %}
}
```

Every struct follows this template. No variation. Zero discretion.

**Metric:** Code style consistency. Target: 100% (all generated code passes `cargo fmt --check` and `cargo clippy` with zero warnings).

### 4. Poka-Yoke (Mistake-Proofing)

**Definition:** Design the process so mistakes are impossible or immediately detectable.

**Traditional manufacturing:** USB plug is asymmetric → can only be inserted one way → no incorrect insertion possible.

**Code generation mapping:**
- **Mistake (SCM):** Developer writes `use crate::adapter::*` in `domain/message.rs` → architecture violation → merged to `master` → discovered weeks later
- **Poka-Yoke (CCM):** `enforce-layers.sh` PreToolUse hook checks every file write; if domain file imports adapter, write is refused with error message

**Poka-Yoke in a2a-rs:**

| Mistake Type | Prevention Mechanism | Detection Time |
|---|---|---|
| Architecture violation (layer imports) | `enforce-layers.sh` hook (PreToolUse) | Before file is written (0s) |
| Invalid ontology | SHACL validation in μ₁ | At generation time (~0.1s) |
| Generated code doesn't compile | `post-edit.sh` hook (PostToolUse) | After file is written (~1s) |
| Three-way inconsistency (spec/ontology/code) | `/spec-check` skill (Phase 4) | On-demand (~5s) |
| Missing derives (Debug, Serialize, etc.) | TaskCompleted agent hook | After agent task completes (~1s) |

**Key property:** All mistakes are caught **before commit**, not during PR review or in production.

**Metric:** Defect escape rate. Target: 0 (no invalid code reaches the repository).

### 5. Andon (Signal Board)

**Definition:** Visual management system that signals abnormalities. Any worker can "pull the Andon cord" to stop the line.

**Traditional manufacturing:** Light turns red above a station → supervisor immediately investigates → problem is resolved before more parts are produced.

**Code generation mapping:**
- **Invisible failure (SCM):** Build fails on CI; logs are buried in GitHub Actions output; developer doesn't notice for hours
- **Andon (CCM):** Hook emits structured status message; user sees real-time feedback; pipeline stops immediately on error

**Andon in a2a-rs:**
```json
// GREEN: Normal operation
{
  "status": "GREEN",
  "stage": "μ₂_extract",
  "progress": 0.42,
  "message": "Constructing structs IR..."
}

// YELLOW: Warning (non-blocking)
{
  "status": "YELLOW",
  "stage": "μ₃_emit",
  "warning": "Template rendered deprecated syntax",
  "suggestion": "Update template to use new macro"
}

// RED: Error (blocking)
{
  "status": "RED",
  "stage": "μ₄_canonicalize",
  "error": "Generated code does not compile",
  "file": "a2a-rs/src/generated/message.rs",
  "line": 42,
  "error_detail": "use of undeclared type `UnknownType`",
  "root_cause": "Ontology references type not defined in any TTL file",
  "action": "Add UnknownType to ontology or fix reference"
}
```

**Hook statusMessage field:** Every hook can set `statusMessage: "..."` to update the user in real-time.

**Metric:** Time-to-awareness of failures. Target: < 1 second (immediate Andon signal).

### 6. Genchi Genbutsu (Go and See)

**Definition:** Go to the actual place (gemba) where work happens. Observe with your own eyes. Don't rely on reports.

**Traditional manufacturing:** Manager goes to factory floor to see the problem firsthand, not reading a report in an office.

**Code generation mapping:**
- **Remote observation (SCM):** Developer reads error logs, stack traces, GitHub issue descriptions → second-hand information → misinterpretation
- **Genchi Genbutsu (CCM):** Agent uses `Explore` skill with `context: fork` to investigate the codebase directly; reads actual files; observes actual behavior

**Genchi Genbutsu in a2a-rs:**
```python
# User reports: "Agent card validation is broken"
# SCM approach: Read issue, guess at cause, propose fix
# CCM approach: Go and see

Task(
  subagent_type="Explore",
  prompt="Navigate to agent card validation code, read implementation, identify validation rules, find where it's called, trace failure",
  context="fork"  # Isolated context window → no pollution of main session
)
```

Agent reads:
1. `a2a-rs/src/domain/core/agent.rs` (AgentCard struct definition)
2. `a2a-rs/src/domain/validation/agent.rs` (validation logic)
3. `a2a-rs/src/adapter/handler/agent.rs` (where validation is called)
4. `a2a-agents/src/reimbursement/agent.rs` (example usage)

Result: Agent observes actual code, not reported symptoms.

**Metric:** Investigation accuracy. Target: 100% (agent identifies root cause, not symptoms).

### 7. Kaizen (Continuous Improvement)

**Definition:** Small, incremental improvements made continuously. Every worker is empowered to suggest improvements.

**Traditional manufacturing:** Workers propose process improvements; best ideas are adopted; standard work is updated.

**Code generation mapping:**
- **Static process (SCM):** Coding conventions documented in CONTRIBUTING.md; rarely updated; no learning mechanism
- **Kaizen (CCM):** Agent memory (`memory: project`) records lessons learned; cross-session persistence; ontology evolves based on discovered constraints

**Kaizen in a2a-rs:**
```markdown
# /root/.claude/projects/-home-user-a2a-rs/memory/MEMORY.md

## Lessons Learned

1. **Ontology naming:** Use `camelCase` for field names, not `snake_case`. Reason: JSON spec uses camelCase; Rust derives use `#[serde(rename_all = "camelCase")]`.

2. **CONSTRUCT query optimization:** Use `OPTIONAL` for optional fields. Reason: Required fields without values cause query to return zero results (silent failure).

3. **Template conditionals:** Always check `field.optional` before rendering `Option<T>`. Reason: Some fields are required in spec but optional in ontology during migration.

4. **Layer violation patterns:** Domain layer can never import from adapter/application/services. Reason: Hexagonal architecture. Detected by `enforce-layers.sh`.

## Ontology Constraints Discovered

- `a2a:Message` must have `a2a:messageType` (required by spec)
- `a2a:TaskState` enum must include `unknown` variant (spec allows server to return unknown states)
- `a2a:AuthScheme` oauth2 variant requires `a2a:flows` property (OWL cardinality constraint)
```

**Agent reads this file at start of every session → incorporates lessons → doesn't repeat mistakes.**

**Metric:** Repeat error rate. Target: 0 (no error occurs twice after being recorded in memory).

### 8. 5 Whys (Root Cause Analysis)

**Definition:** Ask "why" five times to drill down from symptom to root cause.

**Traditional manufacturing:**
- Problem: Machine stopped.
- Why? Overload, fuse blew.
- Why? Bearing not lubricated.
- Why? Lubrication pump not working.
- Why? Pump shaft worn out.
- Why? No strainer, metal scraps got in.
- **Root cause:** Install strainer in pump.

**Code generation mapping:**
- Problem: Generated code doesn't compile.
- Why? Field type is undefined.
- Why? CONSTRUCT query didn't select the type definition.
- Why? WHERE clause pattern didn't match.
- Why? Ontology uses `rdfs:range` but query expects `a2a:fieldType`.
- Why? Ontology was migrated but query wasn't updated.
- **Root cause:** Update query to use `rdfs:range`.

**5 Whys in a2a-rs:**

`/trace-issue` skill implements 5 Whys:
```bash
/trace-issue "AgentCard field 'url' is missing in generated code"

# Trace layer by layer:
1. Check generated code: a2a-rs/src/generated/agent.rs
   - `url` field is not present
2. Check template: ggen/templates/struct.tera
   - Template renders all fields from `struct.hasField`
3. Check IR: ir/structs.ttl
   - IR does not contain `?field a2a:fieldName "url"`
4. Check CONSTRUCT query: ggen/ggen.toml [query.structs]
   - Query selects fields where `?field rdfs:domain ?class`
5. Check ontology: ggen/ontology/a2a-agent.ttl
   - `a2a:url` is defined but `rdfs:domain` is missing

ROOT CAUSE: Ontology triple missing. Fix:
  a2a:url rdfs:domain a2a:AgentCard .
```

**Metric:** Root cause identification time. Target: < 30 seconds (automated trace through 5 layers).

---

## Advanced TPS Concepts

### SMED (Single-Minute Exchange of Die)

**Definition:** Reduce setup time for changing production line from one product to another. Goal: < 10 minutes (single-digit minutes).

**Traditional manufacturing:** Changing from producing Part A to Part B takes 4 hours (die change, calibration, first-piece inspection).

**Code generation mapping:**
- **Setup time (SCM):** Switching from generating Message types to generating Task types requires changing templates, queries, test fixtures → 30 minutes
- **SMED (CCM):** All types use the same pipeline (μ₁–μ₅); switching is instant (just edit a different TTL file)

**SMED in a2a-rs:**
```bash
# Generate Message types
ggen generate --config ggen/ggen.toml --include message

# Switch to Task types (no setup required)
ggen generate --config ggen/ggen.toml --include task

# Setup time: 0 seconds
```

**Key enabler:** Parameterization. Templates are generic over type category (struct, enum, port). CONSTRUCT queries are generic over ontology file. No hard-coded assumptions.

### Takt Time

**Definition:** The rate at which finished products must be completed to meet customer demand.

**Formula:** `Takt Time = Available Production Time / Customer Demand`

**Example:** If customers order 240 units per day and factory runs 8 hours (480 minutes):
- Takt Time = 480 min / 240 units = 2 minutes per unit
- Factory must produce 1 unit every 2 minutes to meet demand

**Code generation mapping:**
- **Customer demand (a2a-rs):** Developer productivity. If developer adds 10 new types per week, generation must complete faster than 10 types/week to avoid backlog.
- **Takt Time (a2a-rs):** 1 week / 10 types = 0.7 days per type. Generation takes ~1 second per type → **Takt Time met with 60,000x margin.**

**Result:** Generation is never the bottleneck. Thinking time (designing the ontology) is the constraint.

### One-Piece Flow

**Definition:** Process one unit completely before starting the next. Minimizes WIP and exposes defects immediately.

**Traditional manufacturing:** Batch processing (assemble 100 units, then paint 100 units, then inspect 100 units) vs. one-piece flow (assemble-paint-inspect unit 1, then unit 2, ...).

**Code generation mapping:**
- **Batch processing (SCM):** Generate 10 types, then compile all 10, then fix all errors → errors compound; hard to isolate root cause
- **One-piece flow (CCM):** Generate 1 type, compile, verify, receipt; then generate next type → errors isolated; root cause obvious

**One-piece flow in a2a-rs:**
```bash
# Anti-pattern (batch)
ggen generate --all  # Generates 50 types at once
cargo check          # 200 compile errors (which types are broken?)

# Best practice (one-piece flow)
ggen generate --include message  # Generate Message type only
cargo check                       # 3 compile errors (isolated to Message)
fix errors
ggen generate --include task      # Generate Task type only
cargo check                       # 1 compile error (isolated to Task)
fix error
# ... continue one type at a time
```

**Benefit:** Defects are caught immediately, not hidden in a batch of 50 types.

### Muda, Mura, Muri (The Three Ms)

**Definitions:**
- **Muda (Waste):** Activities that consume resources but add no value
- **Mura (Unevenness):** Variation in workload or output
- **Muri (Overburden):** Excessive strain on workers or equipment

**Code generation mapping:**

| Concept | SCM Example | CCM Elimination |
|---|---|---|
| **Muda** | Writing boilerplate serialization code for every struct | Generated via `#[derive(Serialize, Deserialize)]` |
| **Muda** | Copy-pasting field definitions from JSON schema to Rust struct | Generated from ontology |
| **Muda** | Writing doc comments that duplicate ontology documentation | Generated from `rdfs:comment` |
| **Mura** | Some PRs take 1 day, others take 2 weeks (unpredictable) | Every ontology change takes ~1 second (uniform) |
| **Mura** | Some developers use `Option<String>`, others use `String` | Template enforces uniform optionality handling |
| **Muri** | Developer must remember 43 coding conventions while writing | Template enforces conventions (zero cognitive load) |
| **Muri** | Developer must manually check spec compliance | `/spec-check` automates three-way validation |

**Result:** All three Ms are eliminated. Pure value-add activities remain.

---

## Metrics and KPIs

### Cycle Time (W)

**Definition:** Time from work item arrival to completion.

**SCM baseline:** 3-7 days for a protocol type change (discussion → spec update → code update → PR → review → merge)

**CCM target:** < 5 minutes for a protocol type change (ontology edit → generate → commit)

**Measurement:** Track git commit timestamps:
```bash
# Time from issue creation to commit
git log --all --grep="Fixes #123" --format="%ct" | xargs -I {} expr {} - $(gh issue view 123 --json createdAt -q .createdAt)
```

### WIP (L)

**Definition:** Number of in-progress tasks (open PRs, unmerged branches, open issues with "in progress" label).

**SCM baseline:** 10-20 open PRs at any time (waiting for review, waiting for fixes, waiting for rebase)

**CCM target:** 0-1 open PRs at any time (ontology changes are committed directly; no PR for generated code)

**Measurement:**
```bash
gh pr list --state open | wc -l
```

### Defect Escape Rate

**Definition:** Number of defects that reach production (invalid types, missing fields, incorrect serialization).

**SCM baseline:** 1-2 defects per release (spec and code drift apart over time)

**CCM target:** 0 defects per release (three-way validation ensures spec/ontology/code are consistent)

**Measurement:**
```bash
# Count issues labeled "bug" that are protocol type related
gh issue list --label bug --search "type definition" --state closed --json number | jq length
```

### Throughput (λ)

**Definition:** Number of protocol type changes per week.

**SCM baseline:** 1-2 type changes per week (limited by review capacity)

**CCM target:** 10-20 type changes per week (limited by thinking time, not coordination time)

**Measurement:**
```bash
git log --since="1 week ago" --grep="ontology" --oneline | wc -l
```

### First-Pass Yield

**Definition:** Percentage of generated code that compiles without manual edits.

**SCM baseline:** N/A (all code is manually written)

**CCM target:** 100% (if ontology is valid, code compiles)

**Measurement:**
```bash
# Run generation, then check if code compiles without manual intervention
ggen generate --all
cargo check --manifest-path a2a-rs/Cargo.toml && echo "100% yield" || echo "Failed"
```

---

## Conclusion

TPS is not a metaphor. It is a **manufacturing operating system** that provides:
- **JIT:** Produce only what's needed (feature flags, on-demand generation)
- **Jidoka:** Build quality in (hooks, validators, receipts)
- **Heijunka:** Level load (parallel CONSTRUCT queries)
- **Kanban:** Pull system (skills load on-demand)
- **Standard Work:** Uniform process (templates)
- **Poka-Yoke:** Mistake-proofing (layer enforcement, compilation checks)
- **Andon:** Immediate feedback (status messages)
- **Genchi Genbutsu:** Go and see (agent investigation)
- **Kaizen:** Continuous improvement (agent memory)
- **5 Whys:** Root cause analysis (/trace-issue skill)

a2a-rs applies all 10 methods to code generation. The result is **zero-defect generated code** with **zero coordination overhead** and **unbounded throughput** (limited by compute, not humans).

This is the manufacturing paradigm applied to software. This is CCM.

---

**Next:** Read [PARADIGM-SHIFT.md](PARADIGM-SHIFT.md) to understand the broader context, or dive into [CONSTRUCT.md](../CONSTRUCT.md) for the theoretical foundations.
