//! PreparedEconomicEffect v1 SPEC VECTORS (CASTLE/SA2A payments).
//!
//! These tests do NOT exercise a2a-rs code. They pin the schema file
//! (`spec/payments/prepared_economic_effect.v1.schema.json`) and the canonical
//! `effect_id` hashing rule against a reference validator in
//! `payments_support/mod.rs`. They are vectors for whoever implements the rule
//! (CASTLE); they would pass regardless of any a2a-rs change and are not a2a-rs
//! coverage. Real a2a-rs coverage is in `payments_a2a_carriage_test.rs`.
//!
//! example.org namespaces are non-authoritative; no ISO 20022 conformance is claimed.

#[path = "payments_support/mod.rs"]
mod support;

use serde_json::{Value, json};
use support::*;

#[test]
fn schema_accepts_canonical_effect_and_rejects_each_missing_field() {
    let v = schema();
    let effect = sample_effect();
    assert_eq!(admit_effect(&effect), Ok(()));
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
    assert_eq!(admit_effect(&extra), Err(Refusal::Schema));
    let mut bad_amount = effect;
    bad_amount.insert("monetary_amount".into(), json!("-1"));
    assert_eq!(admit_effect(&bad_amount), Err(Refusal::Schema));
}

#[test]
fn date_time_format_is_enforced() {
    let mut e = sample_effect();
    e.insert("created_at".into(), json!("yesterday"));
    assert_eq!(
        admit_effect(&e),
        Err(Refusal::Schema),
        "format not enforced"
    );
    let mut e = sample_effect();
    e.insert("expires_at".into(), json!("2026-09-29T00:00:00Z"));
    // Expiry equal to creation: schema-valid, refused by the ordering check.
    // (Identity field changed, so re-derive the id to isolate the ordering rule.)
    let id = compute_effect_id(&e);
    e.insert("effect_id".into(), json!(id));
    assert_eq!(admit_effect(&e), Err(Refusal::ExpiryNotAfterCreation));
}

#[test]
fn every_identity_field_change_yields_effect_identity_mismatch() {
    let effect = sample_effect();
    for field in IDENTITY_FIELDS {
        let mut changed = effect.clone();
        let new = if field == "monetary_amount" {
            "999.99".to_string()
        } else if field == "expires_at" {
            "2026-10-01T00:00:00Z".to_string()
        } else if field.ends_with("digest") {
            digest("other")
        } else {
            format!("{}-changed", effect[field].as_str().unwrap())
        };
        changed.insert(field.into(), json!(new));
        assert_eq!(
            admit_effect(&changed),
            Err(Refusal::EffectIdentityMismatch),
            "changing {field} kept the old effect_id admissible"
        );
    }
}

/// Audit mutation: changing only the beneficiary account (all other identity
/// fields unchanged) must not reuse the old authorization.
#[test]
fn beneficiary_account_swap_is_refused() {
    let mut e = sample_effect();
    e.insert(
        "beneficiary_account_identity".into(),
        json!("acct-attacker"),
    );
    assert_eq!(admit_effect(&e), Err(Refusal::EffectIdentityMismatch));
}

#[test]
fn non_identity_fields_do_not_change_effect_id() {
    let effect = sample_effect();
    let mut c = effect.clone();
    c.insert("nonce".into(), json!("n-2"));
    c.insert("created_at".into(), json!("2026-09-29T01:00:00Z"));
    c.insert("idempotency_key".into(), json!("idem-other"));
    assert_eq!(compute_effect_id(&c), effect["effect_id"].as_str().unwrap());
    assert_eq!(admit_effect(&c), Ok(()));
}

#[test]
fn canonical_hash_vector_is_stable() {
    let id = compute_effect_id(&sample_effect());
    let canon = r#"{"authority_digest":"AD","authority_grant_id":"grant-1","beneficiary_account_identity":"acct-ben-42","beneficiary_id":"ben-42","currency_or_asset":"USD","expires_at":"2026-09-30T00:00:00Z","funding_source_id":"fund-1","monetary_amount":"125.50","obligation_id":"obl-001","payer_role":"payer","policy_profile_id":"policy-a","principal_id":"principal-7","purpose":"invoice-9913","rail_profile_id":"rail-sim-v1","resource_reservation_id":"resv-1"}"#
        .replace("AD", &digest("authority"));
    assert_eq!(
        id,
        hex(&<sha2::Sha256 as sha2::Digest>::digest(canon.as_bytes()))
    );
}
