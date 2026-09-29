//! Shared PreparedEconomicEffect v1 support for the payments tests.
//!
//! NOT a2a-rs code. This is a spec-side reference validator (schema + canonical
//! effect_id hash + RFC 3339 check) used by:
//!  - `payments_spec_vectors.rs` (pure spec vectors, no a2a-rs coverage claimed)
//!  - `payments_a2a_carriage_test.rs` (checks a2a-rs types carry the payload intact)
//!
//! example.org namespaces are non-authoritative; no ISO 20022 conformance is claimed.
#![allow(dead_code)]

use jsonschema::{Draft, Validator};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// Fields whose change must change effect_id. Includes the account, funding and
/// policy fields (v1 hardening: changing the beneficiary account must not reuse
/// an authorization). Excludes only per-submission noise: effect_id itself,
/// idempotency_key, created_at, nonce, parent_receipt, message_profile_version,
/// law_state_digest, counterparty_evidence_digest, canonical_payload_digest.
pub const IDENTITY_FIELDS: [&str; 15] = [
    "principal_id",
    "payer_role",
    "beneficiary_id",
    "beneficiary_account_identity",
    "monetary_amount",
    "currency_or_asset",
    "purpose",
    "authority_grant_id",
    "authority_digest",
    "policy_profile_id",
    "obligation_id",
    "rail_profile_id",
    "expires_at",
    "resource_reservation_id",
    "funding_source_id",
];

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn digest(tag: &str) -> String {
    hex(&Sha256::digest(tag.as_bytes()))
}

/// Canonical form: identity fields only, keys sorted (serde_json Map is a BTreeMap
/// without preserve_order), compact separators.
pub fn compute_effect_id(effect: &Map<String, Value>) -> String {
    let mut canon = Map::new();
    for k in IDENTITY_FIELDS {
        canon.insert(k.to_string(), effect[k].clone());
    }
    hex(&Sha256::digest(
        serde_json::to_vec(&Value::Object(canon)).unwrap(),
    ))
}

pub fn sample_effect() -> Map<String, Value> {
    let mut m = json!({
        "effect_id": "",
        "obligation_id": "obl-001",
        "principal_id": "principal-7",
        "payer_role": "payer",
        "beneficiary_id": "ben-42",
        "beneficiary_account_identity": "acct-ben-42",
        "monetary_amount": "125.50",
        "currency_or_asset": "USD",
        "purpose": "invoice-9913",
        "authority_grant_id": "grant-1",
        "authority_digest": digest("authority"),
        "policy_profile_id": "policy-a",
        "law_state_digest": digest("law"),
        "counterparty_evidence_digest": digest("cp"),
        "funding_source_id": "fund-1",
        "resource_reservation_id": "resv-1",
        "rail_profile_id": "rail-sim-v1",
        "message_profile_version": "1",
        "idempotency_key": "idem-obl-001",
        "created_at": "2026-09-29T00:00:00Z",
        "expires_at": "2026-09-30T00:00:00Z",
        "nonce": "n-1",
        "parent_receipt": null,
        "canonical_payload_digest": digest("payload")
    })
    .as_object()
    .unwrap()
    .clone();
    let id = compute_effect_id(&m);
    m.insert("effect_id".into(), json!(id));
    m
}

pub fn schema() -> Validator {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../spec/payments/prepared_economic_effect.v1.schema.json"
    );
    let raw = std::fs::read_to_string(path).expect("schema file");
    Validator::options()
        .with_draft(Draft::Draft7)
        .should_validate_formats(true)
        .build(&serde_json::from_str::<Value>(&raw).unwrap())
        .expect("schema compiles")
}

#[derive(Debug, PartialEq)]
pub enum Refusal {
    Schema,
    DateTime(&'static str),
    ExpiryNotAfterCreation,
    EffectIdentityMismatch,
}

/// Full admission of an effect payload: JSON Schema (formats enforced), RFC 3339
/// parse of both timestamps (chrono), expiry after creation, and effect_id equal
/// to the canonical hash of the identity fields.
pub fn admit_effect(effect: &Map<String, Value>) -> Result<(), Refusal> {
    if !schema().is_valid(&Value::Object(effect.clone())) {
        return Err(Refusal::Schema);
    }
    let parse = |k: &'static str| {
        chrono::DateTime::parse_from_rfc3339(effect[k].as_str().unwrap())
            .map_err(|_| Refusal::DateTime(k))
    };
    let (created, expires) = (parse("created_at")?, parse("expires_at")?);
    if expires <= created {
        return Err(Refusal::ExpiryNotAfterCreation);
    }
    if compute_effect_id(effect) != effect["effect_id"].as_str().unwrap() {
        return Err(Refusal::EffectIdentityMismatch);
    }
    Ok(())
}
