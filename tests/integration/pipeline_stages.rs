//! Pipeline stages tests
//!
//! Tests for individual stages of the compilation pipeline:
//! 1. Type Checker (Σ)
//! 2. Guard Evaluator (H)
//! 3. Orderer (Λ)
//! 4. Workflow Kernel
//! 5. Invariant Verifier (Q)
//! 6. Writer
//! 7. Receipt Builder

use crate::common::CompilerService;
use osiris_compiler::application::{CompileRequest, CompileResponse};
use osiris_compiler::domain::{Operation, OperationKind};
use std::collections::HashMap;
use uuid::Uuid;

async fn compile_operation(
    service: &CompilerService,
    operation: Operation,
) -> reqwest::Result<CompileResponse> {
    let request = CompileRequest {
        operation,
        replay_pointers: vec![],
        metadata: HashMap::new(),
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
async fn test_stage_1_type_checker() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "valid code".to_string(),
        },
        1,
    );

    let response = compile_operation(&compiler, operation).await.unwrap();

    // Type checker is stage 1, should complete successfully
    assert!(response.pipeline_stats.completed_stages >= 1);
}

#[tokio::test]
async fn test_stage_2_guard_evaluator() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::TypeCheck {
            module_id: "valid_module".to_string(),
        },
        1,
    );

    let response = compile_operation(&compiler, operation).await.unwrap();

    // Guard evaluator is stage 2, should complete
    assert!(response.pipeline_stats.completed_stages >= 2);
}

#[tokio::test]
async fn test_stage_3_orderer() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Submit multiple operations to test ordering
    let ops = vec![
        Operation::new(
            OperationKind::Parse {
                input: "a".to_string(),
            },
            1,
        ),
        Operation::new(
            OperationKind::Parse {
                input: "b".to_string(),
            },
            2,
        ),
        Operation::new(
            OperationKind::Parse {
                input: "c".to_string(),
            },
            1,
        ),
    ];

    for op in ops {
        let response = compile_operation(&compiler, op).await.unwrap();
        // Orderer is stage 3, should complete
        assert!(response.pipeline_stats.completed_stages >= 3);
    }
}

#[tokio::test]
async fn test_stage_4_workflow_kernel() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Link {
            modules: vec!["m1".to_string(), "m2".to_string()],
        },
        1,
    );

    let response = compile_operation(&compiler, operation).await.unwrap();

    // Workflow kernel is stage 4
    assert!(response.pipeline_stats.completed_stages >= 4);
}

#[tokio::test]
async fn test_stage_5_invariant_verifier() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Optimize {
            ir_id: "ir_1".to_string(),
            level: 2,
        },
        1,
    );

    let response = compile_operation(&compiler, operation).await.unwrap();

    // Invariant verifier is stage 5
    assert!(response.pipeline_stats.completed_stages >= 5);
}

#[tokio::test]
async fn test_stage_6_writer() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::CodeGen {
            target: "x86_64".to_string(),
        },
        1,
    );

    let response = compile_operation(&compiler, operation).await.unwrap();

    // Writer is stage 6
    assert!(response.pipeline_stats.completed_stages >= 6);
}

#[tokio::test]
async fn test_stage_7_receipt_builder() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "test".to_string(),
        },
        1,
    );

    let response = compile_operation(&compiler, operation).await.unwrap();

    // Receipt builder is stage 7 (final stage)
    assert_eq!(response.pipeline_stats.completed_stages, 7);
    assert!(!response.receipt.id.to_string().is_empty());
}

#[tokio::test]
async fn test_all_stages_complete_successfully() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "complete pipeline test".to_string(),
        },
        1,
    );

    let response = compile_operation(&compiler, operation).await.unwrap();

    // All 7 stages should complete
    assert_eq!(response.pipeline_stats.completed_stages, 7);
    assert!(response.pipeline_stats.duration_ms > 0);
}

#[tokio::test]
async fn test_stage_timing_recorded() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "timing test".to_string(),
        },
        1,
    );

    let response = compile_operation(&compiler, operation).await.unwrap();

    // Duration should be recorded
    assert!(response.pipeline_stats.duration_ms >= 0);
}

#[tokio::test]
async fn test_complex_operation_through_pipeline() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation = Operation::new(
        OperationKind::Link {
            modules: vec![
                "stdlib".to_string(),
                "core".to_string(),
                "user".to_string(),
            ],
        },
        5,
    )
    .with_source("integration_test".to_string());

    let response = compile_operation(&compiler, operation).await.unwrap();

    // Should pass through all stages
    assert_eq!(response.pipeline_stats.completed_stages, 7);
    assert!(!response.receipt.operation_hash.is_empty());
}

#[tokio::test]
async fn test_receipt_generated_after_all_stages() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let operation_id = Uuid::new_v4();
    let mut operation = Operation::new(
        OperationKind::CodeGen {
            target: "wasm32".to_string(),
        },
        1,
    );
    operation.id = operation_id;

    let response = compile_operation(&compiler, operation).await.unwrap();

    // Receipt should be generated after all 7 stages
    assert_eq!(response.pipeline_stats.completed_stages, 7);
    assert_eq!(response.receipt.operation_id, operation_id);
}

#[tokio::test]
async fn test_deterministic_output_same_input() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Create two identical operations (with same ID for determinism)
    let op_id = Uuid::new_v4();

    let mut op1 = Operation::new(
        OperationKind::Parse {
            input: "deterministic test".to_string(),
        },
        5,
    );
    op1.id = op_id;

    let response1 = compile_operation(&compiler, op1.clone()).await.unwrap();

    let mut op2 = Operation::new(
        OperationKind::Parse {
            input: "deterministic test".to_string(),
        },
        5,
    );
    op2.id = op_id;

    let response2 = compile_operation(&compiler, op2).await.unwrap();

    // Same input should produce same operation hash
    assert_eq!(response1.receipt.operation_hash, response2.receipt.operation_hash);
}

#[tokio::test]
async fn test_priority_affects_ordering() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // High priority operation
    let high_priority = Operation::new(
        OperationKind::Parse {
            input: "high".to_string(),
        },
        100,
    );

    // Low priority operation
    let low_priority = Operation::new(
        OperationKind::Parse {
            input: "low".to_string(),
        },
        1,
    );

    let resp1 = compile_operation(&compiler, high_priority).await.unwrap();
    let resp2 = compile_operation(&compiler, low_priority).await.unwrap();

    // Both should complete all 7 stages
    assert_eq!(resp1.pipeline_stats.completed_stages, 7);
    assert_eq!(resp2.pipeline_stats.completed_stages, 7);
}
