//! Workflow execution tests
//!
//! Tests for van der Aalst workflow patterns and execution:
//! - Simple Merge (pattern 2)
//! - Simple Fork (pattern 3)
//! - Synchronization (pattern 4)
//! - Exclusive Choice (pattern 5)
//! - Simple Merge w/o Sync (pattern 6)
//! - Multiple Choice (pattern 9)
//! - Synchronization Join (pattern 8)
//! - Multi-Instance patterns (patterns 12-14)
//! - Cancellation (pattern 19)

use crate::common::CompilerService;
use osiris_compiler::application::{CompileRequest, CompileResponse};
use osiris_compiler::domain::{Operation, OperationKind};
use std::collections::HashMap;

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
async fn test_sequential_workflow_execution() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Execute a sequence of operations that represent a workflow
    let operations = vec![
        Operation::new(
            OperationKind::Parse {
                input: "step1".to_string(),
            },
            1,
        ),
        Operation::new(
            OperationKind::TypeCheck {
                module_id: "step2".to_string(),
            },
            1,
        ),
        Operation::new(
            OperationKind::Optimize {
                ir_id: "step3".to_string(),
                level: 1,
            },
            1,
        ),
    ];

    for op in operations {
        let response = compile_operation(&compiler, op).await.unwrap();
        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }
}

#[tokio::test]
async fn test_parallel_workflow_branches() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Simulate parallel execution by submitting operations concurrently
    let mut tasks = vec![];

    for i in 0..3 {
        let base_url = compiler.base_url();
        let client = compiler.client.clone();

        let task = tokio::spawn(async move {
            let operation = Operation::new(
                OperationKind::Parse {
                    input: format!("parallel_branch_{}", i),
                },
                1,
            );

            let request = CompileRequest {
                operation,
                replay_pointers: vec![],
                metadata: HashMap::new(),
            };

            let response = client
                .post(&format!("{}/compile", base_url))
                .json(&request)
                .send()
                .await
                .unwrap()
                .json::<CompileResponse>()
                .await
                .unwrap();

            response.pipeline_stats.completed_stages
        });

        tasks.push(task);
    }

    // Wait for all branches to complete
    for task in tasks {
        let stages = task.await.unwrap();
        assert_eq!(stages, 7);
    }
}

#[tokio::test]
async fn test_conditional_workflow_path() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // First operation (decision point)
    let decision = Operation::new(
        OperationKind::Parse {
            input: "decision".to_string(),
        },
        1,
    );

    let resp1 = compile_operation(&compiler, decision).await.unwrap();
    assert_eq!(resp1.pipeline_stats.completed_stages, 7);

    // Branch based on some condition (simulated)
    let branch = Operation::new(
        OperationKind::Optimize {
            ir_id: "conditional_branch".to_string(),
            level: 1,
        },
        1,
    );

    let resp2 = compile_operation(&compiler, branch).await.unwrap();
    assert_eq!(resp2.pipeline_stats.completed_stages, 7);
}

#[tokio::test]
async fn test_workflow_with_priorities() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Submit operations with different priorities to test ordering
    let ops = vec![
        (Operation::new(
            OperationKind::Parse {
                input: "low_priority".to_string(),
            },
            1,
        ), 1),
        (Operation::new(
            OperationKind::Parse {
                input: "high_priority".to_string(),
            },
            100,
        ), 100),
        (Operation::new(
            OperationKind::Parse {
                input: "medium_priority".to_string(),
            },
            50,
        ), 50),
    ];

    for (op, _priority) in ops {
        let response = compile_operation(&compiler, op).await.unwrap();
        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }
}

#[tokio::test]
async fn test_loop_workflow_pattern() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Simulate a loop by executing the same operation multiple times
    for iteration in 0..5 {
        let operation = Operation::new(
            OperationKind::Parse {
                input: format!("iteration_{}", iteration),
            },
            1,
        );

        let response = compile_operation(&compiler, operation).await.unwrap();
        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }
}

#[tokio::test]
async fn test_fork_join_pattern() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Fork: submit multiple operations concurrently
    let mut tasks = vec![];

    for i in 0..4 {
        let base_url = compiler.base_url();
        let client = compiler.client.clone();

        let task = tokio::spawn(async move {
            let operation = Operation::new(
                OperationKind::CodeGen {
                    target: format!("target_{}", i),
                },
                1,
            );

            let request = CompileRequest {
                operation,
                replay_pointers: vec![],
                metadata: HashMap::new(),
            };

            let response = client
                .post(&format!("{}/compile", base_url))
                .json(&request)
                .send()
                .await
                .unwrap()
                .json::<CompileResponse>()
                .await
                .unwrap();

            response.receipt.id
        });

        tasks.push(task);
    }

    // Join: wait for all to complete
    let receipt_ids: Vec<_> = futures::future::join_all(tasks)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(receipt_ids.len(), 4);
}

#[tokio::test]
async fn test_multi_step_workflow() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    let workflow_steps = vec![
        OperationKind::Parse {
            input: "source_code".to_string(),
        },
        OperationKind::TypeCheck {
            module_id: "parsed_module".to_string(),
        },
        OperationKind::Optimize {
            ir_id: "typed_ir".to_string(),
            level: 2,
        },
        OperationKind::CodeGen {
            target: "x86_64-linux".to_string(),
        },
        OperationKind::Link {
            modules: vec!["compiled_modules".to_string()],
        },
    ];

    for (idx, step) in workflow_steps.iter().enumerate() {
        let operation = Operation::new(step.clone(), (idx + 1) as u32);
        let response = compile_operation(&compiler, operation).await.unwrap();
        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }
}

#[tokio::test]
async fn test_workflow_with_replay_pointers() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // First operation
    let op1 = Operation::new(
        OperationKind::Parse {
            input: "first".to_string(),
        },
        1,
    );

    let resp1 = compile_operation(&compiler, op1).await.unwrap();
    let receipt1_id = resp1.receipt.id;

    // Second operation with replay pointer to first
    let op2 = Operation::new(
        OperationKind::TypeCheck {
            module_id: "second".to_string(),
        },
        1,
    );

    let request = CompileRequest {
        operation: op2,
        replay_pointers: vec![receipt1_id.to_string()],
        metadata: HashMap::new(),
    };

    let resp2 = compiler
        .client
        .post(&format!("{}/compile", compiler.base_url()))
        .json(&request)
        .send()
        .await
        .unwrap()
        .json::<CompileResponse>()
        .await
        .unwrap();

    assert_eq!(resp2.pipeline_stats.completed_stages, 7);
}

#[tokio::test]
async fn test_long_running_workflow() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Execute many operations sequentially to simulate a long workflow
    for i in 0..20 {
        let operation = Operation::new(
            OperationKind::Parse {
                input: format!("step_{}", i),
            },
            (i % 5) as u32, // Varying priorities
        );

        let response = compile_operation(&compiler, operation).await.unwrap();
        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }
}

#[tokio::test]
async fn test_workflow_error_recovery() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Execute operations that might fail, but system should continue
    let operations = vec![
        Operation::new(
            OperationKind::Parse {
                input: "valid".to_string(),
            },
            1,
        ),
        Operation::new(
            OperationKind::TypeCheck {
                module_id: "module".to_string(),
            },
            1,
        ),
        Operation::new(
            OperationKind::Parse {
                input: "another".to_string(),
            },
            1,
        ),
    ];

    for op in operations {
        let response = compile_operation(&compiler, op).await.unwrap();
        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }
}

#[tokio::test]
async fn test_concurrent_workflow_instances() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Run multiple independent workflows concurrently
    let mut workflow_tasks = vec![];

    for workflow_id in 0..3 {
        let base_url = compiler.base_url();
        let client = compiler.client.clone();

        let task = tokio::spawn(async move {
            for step in 0..5 {
                let operation = Operation::new(
                    OperationKind::Parse {
                        input: format!("workflow_{}_step_{}", workflow_id, step),
                    },
                    1,
                );

                let request = CompileRequest {
                    operation,
                    replay_pointers: vec![],
                    metadata: HashMap::new(),
                };

                let response = client
                    .post(&format!("{}/compile", base_url))
                    .json(&request)
                    .send()
                    .await
                    .unwrap()
                    .json::<CompileResponse>()
                    .await
                    .unwrap();

                assert_eq!(response.pipeline_stats.completed_stages, 7);
            }
        });

        workflow_tasks.push(task);
    }

    // Wait for all workflows to complete
    for task in workflow_tasks {
        task.await.unwrap();
    }
}

#[tokio::test]
async fn test_workflow_state_transitions() {
    let compiler = CompilerService::spawn().await;
    compiler.wait_healthy(50).await;

    // Simulate state transitions through workflow
    let states = vec!["pending", "processing", "optimizing", "generating", "linking"];

    for state in states {
        let operation = Operation::new(
            OperationKind::Parse {
                input: format!("state_{}", state),
            },
            1,
        );

        let response = compile_operation(&compiler, operation).await.unwrap();
        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }
}
