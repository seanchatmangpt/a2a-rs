//! Payments conformance vectors (CASTLE/SA2A peer role).
//!
//! a2a-rs is the A2A protocol-surface reference: it can carry an economic
//! effect, but carrying is not authority. These vectors pin four properties:
//!  - the PreparedEconomicEffect v1 JSON Schema (spec/payments) is enforced,
//!  - effect_id is a canonical hash of the identity fields (F2),
//!  - the effect survives an A2A Message DataPart round trip unchanged (F9),
//!  - accept-then-drop-ACK yields UNKNOWN, never a blind retry (F3, F5).
//!
//! example.org namespaces are non-authoritative; no ISO 20022 conformance is claimed.

use a2a_rs::domain::{Message, Part, TaskState};
use jsonschema::{Draft, Validator};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// Fields whose change must change effect_id.
const IDENTITY_FIELDS: [&str; 10] = [
    "principal_id",
    "beneficiary_id",
    "monetary_amount",
    "currency_or_asset",
    "purpose",
    "authority_grant_id",
    "obligation_id",
    "rail_profile_id",
    "expires_at",
    "resource_reservation_id",
];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Canonical form: identity fields only, keys sorted (serde_json Map is a BTreeMap
/// without preserve_order), compact separators.
fn compute_effect_id(effect: &Map<String, Value>) -> String {
    let mut canon = Map::new();
    for k in IDENTITY_FIELDS {
        canon.insert(k.to_string(), effect[k].clone());
    }
    hex(&Sha256::digest(
        serde_json::to_vec(&Value::Object(canon)).unwrap(),
    ))
}

#[derive(Debug, PartialEq)]
enum Refusal {
    EffectIdentityMismatch,
}

/// An authorization names an effect_id; it only covers an effect that hashes to it.
fn authorize(granted_effect_id: &str, effect: &Map<String, Value>) -> Result<(), Refusal> {
    if compute_effect_id(effect) == granted_effect_id {
        Ok(())
    } else {
        Err(Refusal::EffectIdentityMismatch)
    }
}

fn digest(tag: &str) -> String {
    hex(&Sha256::digest(tag.as_bytes()))
}

fn sample_effect() -> Map<String, Value> {
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

fn schema() -> Validator {
    let raw = std::fs::read_to_string("../spec/payments/prepared_economic_effect.v1.schema.json")
        .expect("schema file");
    Validator::options()
        .with_draft(Draft::Draft7)
        .build(&serde_json::from_str::<Value>(&raw).unwrap())
        .expect("schema compiles")
}

#[test]
fn schema_accepts_canonical_effect_and_rejects_each_missing_field() {
    let v = schema();
    let effect = sample_effect();
    assert!(v.is_valid(&Value::Object(effect.clone())));
    for key in effect.keys() {
        let mut broken = effect.clone();
        broken.remove(key);
        assert!(
            !v.is_valid(&Value::Object(broken)),
            "missing {key} accepted"
        );
    }
    let mut extra = effect.clone();
    extra.insert("widened_authority".into(), json!(true));
    assert!(
        !v.is_valid(&Value::Object(extra)),
        "additional property accepted"
    );
    let mut bad_amount = effect;
    bad_amount.insert("monetary_amount".into(), json!("-1"));
    assert!(!v.is_valid(&Value::Object(bad_amount)));
}

#[test]
fn identity_change_yields_effect_identity_mismatch() {
    let effect = sample_effect();
    let granted = effect["effect_id"].as_str().unwrap().to_string();
    assert_eq!(authorize(&granted, &effect), Ok(()));
    for field in IDENTITY_FIELDS {
        let mut changed = effect.clone();
        changed.insert(
            field.into(),
            json!(format!("{}-changed", effect[field].as_str().unwrap())),
        );
        assert_eq!(
            authorize(&granted, &changed),
            Err(Refusal::EffectIdentityMismatch),
            "changing {field} reused the old authorization"
        );
    }
}

#[test]
fn non_identity_fields_do_not_change_effect_id() {
    let effect = sample_effect();
    let mut c = effect.clone();
    c.insert("nonce".into(), json!("n-2"));
    c.insert("created_at".into(), json!("2026-09-29T01:00:00Z"));
    c.insert("idempotency_key".into(), json!("idem-other"));
    assert_eq!(compute_effect_id(&c), effect["effect_id"].as_str().unwrap());
}

#[test]
fn canonical_hash_vector_is_stable() {
    // Pinned vector: a change in canonicalisation must be a deliberate, visible edit.
    let id = compute_effect_id(&sample_effect());
    assert_eq!(id.len(), 64);
    assert_eq!(id, compute_effect_id(&sample_effect()));
    let canon = r#"{"authority_grant_id":"grant-1","beneficiary_id":"ben-42","currency_or_asset":"USD","expires_at":"2026-09-30T00:00:00Z","monetary_amount":"125.50","obligation_id":"obl-001","principal_id":"principal-7","purpose":"invoice-9913","rail_profile_id":"rail-sim-v1","resource_reservation_id":"resv-1"}"#;
    assert_eq!(id, hex(&Sha256::digest(canon.as_bytes())));
}

#[test]
fn effect_survives_a2a_message_data_part_round_trip() {
    let effect = sample_effect();
    let mut msg = Message::agent_text("prepared effect".into(), "msg-1".into());
    msg.add_part(Part::data(effect.clone()));
    let wire = serde_json::to_string(&msg).unwrap();
    let back: Message = serde_json::from_str(&wire).unwrap();
    let carried = back
        .parts
        .iter()
        .find_map(|p| match p {
            Part::Data { data, .. } => Some(data.clone()),
            _ => None,
        })
        .expect("data part");
    assert_eq!(carried, effect);
    assert!(schema().is_valid(&Value::Object(carried.clone())));
    // Carrying grants nothing: the id still binds to the same identity.
    assert_eq!(
        compute_effect_id(&carried),
        carried["effect_id"].as_str().unwrap()
    );
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Settlement {
    Prepared,
    Submitted,
    Unknown,
    Accepted,
    Rejected,
    Settled,
}

#[derive(Debug, PartialEq)]
enum Step {
    Ack,
    Timeout,
    Reject,
    Final,
}

fn advance(s: Settlement, step: Step) -> Settlement {
    match (s, step) {
        (Settlement::Prepared, _) => Settlement::Submitted,
        (Settlement::Submitted, Step::Timeout) => Settlement::Unknown,
        (Settlement::Submitted | Settlement::Unknown, Step::Ack) => Settlement::Accepted,
        (Settlement::Submitted | Settlement::Unknown, Step::Reject) => Settlement::Rejected,
        (Settlement::Accepted, Step::Final) => Settlement::Settled,
        (s, _) => s,
    }
}

/// A new submission is eligible only after reconciliation proved non-acceptance.
fn may_resubmit(s: Settlement) -> bool {
    s == Settlement::Rejected
}

#[test]
fn accept_then_drop_ack_is_unknown_and_never_blindly_retried() {
    // Rail accepted, ACK dropped: the client only sees a timeout.
    let s = advance(Settlement::Prepared, Step::Ack);
    assert_eq!(s, Settlement::Submitted);
    let s = advance(s, Step::Timeout);
    assert_eq!(s, Settlement::Unknown);
    assert!(!may_resubmit(s), "F3: timeout permitted blind retry");
    // A settled state is unreachable without observed acceptance then finality (F5).
    assert_ne!(advance(s, Step::Final), Settlement::Settled);
    let s = advance(advance(s, Step::Ack), Step::Final);
    assert_eq!(s, Settlement::Settled);
    // Reconciliation proving non-acceptance is the only path to resubmission.
    let r = advance(advance(Settlement::Prepared, Step::Ack), Step::Timeout);
    assert!(may_resubmit(advance(r, Step::Reject)));
}

#[test]
fn a2a_unknown_task_state_carries_unknown_outcome() {
    let v = serde_json::to_value(TaskState::Unknown).unwrap();
    assert_eq!(v, json!("unknown"));
    assert_eq!(
        serde_json::from_value::<TaskState>(v).unwrap(),
        TaskState::Unknown
    );
    // Unknown is not terminal-success: it must not equal Completed.
    assert_ne!(TaskState::Unknown, TaskState::Completed);
}
