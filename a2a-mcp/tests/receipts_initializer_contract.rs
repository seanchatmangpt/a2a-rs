//! Guard: Task/AgentCard struct-literal initializers must stay exhaustive under
//! the crypto feature. Upstream gates `receipts: Option<Vec<Receipt>>` behind
//! `#[cfg(feature = "crypto")]`; this crate enables crypto unconditionally
//! (see Cargo.toml), so a literal that omits the field fails to compile. These
//! tests exercise that contract directly — they fail at compile time if the
//! field set drifts again.

use a2a_rs::{AgentCard, Task};

#[test]
fn task_initializer_covers_receipts_field() {
    let task = Task {
        id: "guard-task".to_string(),
        context_id: "guard-ctx".to_string(),
        status: a2a_rs::TaskStatus {
            state: a2a_rs::TaskState::Submitted,
            message: None,
            timestamp: None,
        },
        artifacts: None,
        history: None,
        metadata: None,
        kind: "task".to_string(),
        receipts: None,
    };
    assert!(task.receipts.is_none());
}

#[test]
fn agent_card_initializer_covers_receipts_field() {
    let card = AgentCard {
        name: "guard".to_string(),
        description: "guard card".to_string(),
        url: "https://example.com/agent".to_string(),
        provider: None,
        version: "0.0.0".to_string(),
        protocol_version: "0.3.0".to_string(),
        preferred_transport: "JSONRPC".to_string(),
        additional_interfaces: None,
        icon_url: None,
        documentation_url: None,
        capabilities: a2a_rs::AgentCapabilities {
            streaming: false,
            push_notifications: false,
            state_transition_history: false,
            extensions: None,
        },
        security_schemes: None,
        security: None,
        default_input_modes: vec![],
        default_output_modes: vec![],
        skills: vec![],
        signatures: None,
        supports_authenticated_extended_card: None,
        receipts: None,
    };
    assert!(card.receipts.is_none());
}
