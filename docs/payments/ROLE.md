# Payments Role: a2a-rs

a2a-rs is the A2A protocol-surface reference and conformance peer for the CASTLE/SA2A
authority plane. It carries economic effects; it never grants authority.

## Deliverable

- `spec/payments/prepared_economic_effect.v1.schema.json`: PreparedEconomicEffect v1
  (CASTLE operational extension, not FIBO; example.org ids are non-authoritative).
- `a2a-rs/tests/payments_effect_conformance_test.rs`: schema, canonical `effect_id`
  hash vectors, A2A DataPart round trip, accept-then-drop-ACK state vectors.

## Falsifier map

| Falsifier | Vector |
|---|---|
| F2 changed beneficiary/amount reuses authorization | `identity_change_yields_effect_identity_mismatch` |
| F3 timeout permits blind retry | `accept_then_drop_ack_is_unknown_and_never_blindly_retried` |
| F5 SETTLED without observed finality | same test, `Final` without `Ack` |
| F7 adapter widens authority | schema `additionalProperties: false` |
| F9 receipt cannot reconstruct effect | DataPart round trip test |

F1, F4, F6, F8, F10 are outside this repo's surface. No ISO 20022 conformance is claimed.
