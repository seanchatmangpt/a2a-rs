//! What a2a-rs actually does for payments: CARRY a PreparedEconomicEffect.
//!
//! Every test here goes through a2a-rs types (`Message`, `Part`, `Task`,
//! `TaskStatus`, `TaskState`) and their serde. Claims are limited to:
//!  - a DataPart payload survives Message and Task JSON round trips unchanged,
//!  - the carried payload still passes the spec admission (schema + hash),
//!  - `TaskState::Unknown` exists, serialises as "unknown", and is not `Completed`.
//! a2a-rs has no payment authorization, idempotency, retry or settlement logic;
//! nothing here asserts any. Spec-only vectors live in `payments_spec_vectors.rs`.
//!
//! example.org namespaces are non-authoritative; no ISO 20022 conformance is claimed.

#[path = "payments_support/mod.rs"]
mod support;

use a2a_rs::domain::{Message, Part, Task, TaskState};
use serde_json::{Map, Value, json};
use support::*;

fn data_part_of(msg: &Message) -> Map<String, Value> {
    msg.parts
        .iter()
        .find_map(|p| match p {
            Part::Data { data, .. } => Some(data.clone()),
            _ => None,
        })
        .expect("data part")
}

#[test]
fn effect_survives_a2a_message_data_part_round_trip() {
    let effect = sample_effect();
    let mut msg = Message::agent_text("prepared effect".into(), "msg-1".into());
    msg.add_part(Part::data(effect.clone()));
    let back: Message = serde_json::from_str(&serde_json::to_string(&msg).unwrap()).unwrap();
    let carried = data_part_of(&back);
    assert_eq!(carried, effect);
    assert_eq!(admit_effect(&carried), Ok(()));
}

#[test]
fn tampering_with_carried_effect_is_detected_by_spec_admission() {
    // The transport does not protect the payload; admission must.
    let mut msg = Message::agent_text("prepared effect".into(), "msg-2".into());
    msg.add_part(Part::data(sample_effect()));
    let mut wire: Value = serde_json::to_value(&msg).unwrap();
    let data = wire["parts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["kind"] == "data")
        .expect("data part on the wire");
    data["data"]["beneficiary_account_identity"] = json!("acct-attacker");
    let back: Message = serde_json::from_value(wire).unwrap();
    assert_eq!(
        admit_effect(&data_part_of(&back)),
        Err(Refusal::EffectIdentityMismatch)
    );
}

#[test]
fn effect_survives_task_status_update_and_task_json_round_trip() {
    let effect = sample_effect();
    let mut msg = Message::agent_text("effect submitted, outcome unknown".into(), "msg-3".into());
    msg.add_part(Part::data(effect.clone()));
    let mut task = Task::new("task-1".into(), "ctx-1".into());
    task.update_status(TaskState::Unknown, Some(msg));

    assert_eq!(task.status.state, TaskState::Unknown);
    assert_ne!(task.status.state, TaskState::Completed);
    let back: Task = serde_json::from_str(&serde_json::to_string(&task).unwrap()).unwrap();
    assert_eq!(back.status.state, TaskState::Unknown);
    let status_msg = back.status.message.as_ref().expect("status message");
    assert_eq!(data_part_of(status_msg), effect);
    // update_status also appends the message to history.
    let hist = back.history.as_ref().expect("history");
    assert_eq!(hist.len(), 1);
    assert_eq!(data_part_of(&hist[0]), effect);
}

#[test]
fn a2a_unknown_task_state_wire_form() {
    let v = serde_json::to_value(TaskState::Unknown).unwrap();
    assert_eq!(v, json!("unknown"));
    assert_eq!(
        serde_json::from_value::<TaskState>(v).unwrap(),
        TaskState::Unknown
    );
    assert_ne!(TaskState::Unknown, TaskState::Completed);
}
