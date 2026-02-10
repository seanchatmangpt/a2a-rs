//! Basic compilation endpoint tests
//!
//! Tests for submitting operations to the compiler and verifying responses

use crate::common::CompilerService;
use osiris_compiler::application::{CompileRequest, CompileResponse};
use osiris_compiler::domain::{Operation, OperationKind};
use serde_json::json;
use uuid::Uuid;

/// Submit a compile request and get the response
async fn compile_operation(
    service: &CompilerService,
    operation: Operation,
) -> reqwest::Result<CompileResponse> {
    let request = CompileRequest {
        operation,
        replay_pointers: vec![],
        metadata: Default::default(),
    };

    service
        .client
        .post(&format!("{}/compile", service.base_url()))
        .json(&request)
        .send()
        .await?
        .json()
        .await
}

#[tokio::test]
async fn test_compile_parse_operation() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "let x = 42;".to_string(),
        },
        1,
    );

    let response = compile_operation(&compiler, operation)
        .await
        .expect("Failed to compile");

    // Verify response structure
    assert!(!response.receipt.id.to_string().is_empty());
    assert!(!response.receipt.operation_hash.is_empty());
    assert_eq!(response.pipeline_stats.completed_stages, 7);
}

#[tokio::test]
async fn test_compile_typecheck_operation() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::TypeCheck {
            module_id: "module_1".to_string(),
        },
        2,
    );

    let response = compile_operation(&compiler, operation)
        .await
        .expect("Failed to compile");

    assert_eq!(response.pipeline_stats.completed_stages, 7);
    assert!(!response.receipt.operation_hash.is_empty());
}

#[tokio::test]
async fn test_compile_optimize_operation() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Optimize {
            ir_id: "ir_1".to_string(),
            level: 2,
        },
        1,
    );

    let response = compile_operation(&compiler, operation)
        .await
        .expect("Failed to compile");

    assert_eq!(response.pipeline_stats.completed_stages, 7);
}

#[tokio::test]
async fn test_compile_codegen_operation() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::CodeGen {
            target: "x86_64-unknown-linux-gnu".to_string(),
        },
        1,
    );

    let response = compile_operation(&compiler, operation)
        .await
        .expect("Failed to compile");

    assert_eq!(response.pipeline_stats.completed_stages, 7);
}

#[tokio::test]
async fn test_compile_link_operation() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Link {
            modules: vec!["mod1".to_string(), "mod2".to_string()],
        },
        3,
    );

    let response = compile_operation(&compiler, operation)
        .await
        .expect("Failed to compile");

    assert_eq!(response.pipeline_stats.completed_stages, 7);
}

#[tokio::test]
async fn test_receipt_has_valid_structure() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "test".to_string(),
        },
        1,
    );
    let operation_id = operation.id;

    let response = compile_operation(&compiler, operation)
        .await
        .expect("Failed to compile");

    // Verify receipt structure
    let receipt = &response.receipt;
    assert!(!receipt.id.to_string().is_empty());
    assert_eq!(receipt.operation_id, operation_id);
    assert!(!receipt.operation_hash.is_empty());
    assert!(!receipt.attestation_hash.is_empty());
    assert_eq!(receipt.operation_hash, receipt.attestation_hash);
}

#[tokio::test]
async fn test_pipeline_stats_tracked() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "test".to_string(),
        },
        1,
    );

    let response = compile_operation(&compiler, operation)
        .await
        .expect("Failed to compile");

    // Verify pipeline stats
    assert!(response.pipeline_stats.duration_ms > 0);
    assert!(response.pipeline_stats.completed_stages > 0);
    assert_eq!(response.pipeline_stats.completed_stages, 7);
    assert!(!response.pipeline_stats.timestamp.is_empty());
}

#[tokio::test]
async fn test_multiple_operations_in_sequence() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let ops = vec![
        Operation::new(
            OperationKind::Parse {
                input: "let x = 1;".to_string(),
            },
            1,
        ),
        Operation::new(
            OperationKind::TypeCheck {
                module_id: "m1".to_string(),
            },
            2,
        ),
        Operation::new(
            OperationKind::Optimize {
                ir_id: "ir1".to_string(),
                level: 1,
            },
            1,
        ),
    ];

    for op in ops {
        let response = compile_operation(&compiler, op)
            .await
            .expect("Failed to compile");
        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }
}

#[tokio::test]
async fn test_operation_with_source_identifier() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "test".to_string(),
        },
        1,
    )
    .with_source("integration-test".to_string());

    let response = compile_operation(&compiler, operation)
        .await
        .expect("Failed to compile");

    assert_eq!(response.pipeline_stats.completed_stages, 7);
}

#[tokio::test]
async fn test_replay_pointers_in_request() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "test".to_string(),
        },
        1,
    );

    let request = CompileRequest {
        operation,
        replay_pointers: vec![Uuid::new_v4().to_string()],
        metadata: Default::default(),
    };

    let response = compiler
        .client
        .post(&format!("{}/compile", compiler.base_url()))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request")
        .json::<CompileResponse>()
        .await
        .expect("Failed to parse response");

    assert_eq!(response.pipeline_stats.completed_stages, 7);
}

#[tokio::test]
async fn test_metadata_in_request() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let mut metadata = std::collections::HashMap::new();
    metadata.insert(
        "correlation_id".to_string(),
        json!("corr-123"),
    );
    metadata.insert(
        "source_system".to_string(),
        json!("integration-test"),
    );

    let operation = Operation::new(
        OperationKind::Parse {
            input: "test".to_string(),
        },
        1,
    );

    let request = CompileRequest {
        operation,
        replay_pointers: vec![],
        metadata,
    };

    let response = compiler
        .client
        .post(&format!("{}/compile", compiler.base_url()))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request")
        .json::<CompileResponse>()
        .await
        .expect("Failed to parse response");

    assert_eq!(response.pipeline_stats.completed_stages, 7);
}

#[tokio::test]
async fn test_high_priority_operation() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "test".to_string(),
        },
        999, // High priority
    );

    let response = compile_operation(&compiler, operation)
        .await
        .expect("Failed to compile");

    assert_eq!(response.pipeline_stats.completed_stages, 7);
}
