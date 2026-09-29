# Payments Role: a2a-rs

a2a-rs is an A2A protocol library. For CASTLE/SA2A payments it can only carry a
PreparedEconomicEffect payload; it has no authorization, idempotency, retry or
settlement logic and this repo claims none.

## Deliverables

- `spec/payments/prepared_economic_effect.v1.schema.json`: PreparedEconomicEffect v1
  (CASTLE operational extension, not FIBO; example.org ids are non-authoritative).
  The schema cannot check `effect_id` against its identity fields; the reference
  validator does. `format: date-time` is enforced only with format validation on
  (the reference validator turns it on and also parses RFC 3339 with chrono).
- `a2a-rs/tests/payments_a2a_carriage_test.rs`: real a2a-rs coverage. Message/Part::data
  and Task/TaskStatus JSON round trips of the effect; `TaskState::Unknown` wire form.
- `a2a-rs/tests/payments_spec_vectors.rs` + `payments_support/mod.rs`: SPEC VECTORS,
  not a2a-rs coverage. Schema file checks and the canonical `effect_id` hash rule
  (15 identity fields, including beneficiary account, payer role, funding source,
  authority digest and policy profile). They pass independently of a2a-rs behavior.

## What is exercised

| Property | Where | a2a-rs code exercised |
|---|---|---|
| Effect payload survives Message/Part::data round trip | carriage test | yes |
| Effect survives Task::update_status + Task JSON round trip (status and history) | carriage test | yes |
| Carried payload tampering is caught by spec admission | carriage test | serde only; detection is the spec validator |
| `TaskState::Unknown` is distinct from `Completed`, serialises as "unknown" | carriage test | yes |
| Changed identity field (incl. beneficiary account) changes `effect_id` | spec vectors | no |
| Schema required fields, additionalProperties false, amount pattern, date-time | spec vectors | no |

## Not covered by this repo

F1, F3 (timeout must not permit blind retry), F4, F5 (settled without finality), F6,
F7 (adapter widening authority), F8, F9 (receipt reconstructing an effect), F10.
a2a-rs has no code surface for these; earlier local-helper tests for F3/F5 were removed
because they exercised only their own helpers. No ISO 20022 conformance is claimed.
