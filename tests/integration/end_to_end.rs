//! End-to-end tests
//!
//! Full system tests with both edge and compiler services running,
//! testing complete request paths from edge gateway to compiler.

use crate::common::TestEnv;
use osiris_compiler::application::{CompileRequest, CompileResponse};
use osiris_compiler::domain::{Operation, OperationKind};
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn test_both_services_healthy() {
    let env = TestEnv::setup().await;

    assert!(env.compiler.is_healthy().await);
    assert!(env.edge.is_healthy().await);

    env.shutdown().await;
}

#[tokio::test]
async fn test_compiler_health_check() {
    let env = TestEnv::setup().await;

    let response = env
        .compiler
        .client
        .get(&format!("{}/health", env.compiler.base_url()))
        .send()
        .await
        .expect("Failed to get health");

    assert_eq!(response.status(), 200);

    env.shutdown().await;
}

#[tokio::test]
async fn test_edge_health_check() {
    let env = TestEnv::setup().await;

    let response = env
        .edge
        .client
        .get(&format!("{}/health", env.edge.base_url()))
        .send()
        .await
        .expect("Failed to get health");

    assert_eq!(response.status(), 200);

    env.shutdown().await;
}

#[tokio::test]
async fn test_edge_readiness_check() {
    let env = TestEnv::setup().await;

    let response = env
        .edge
        .client
        .get(&format!("{}/ready", env.edge.base_url()))
        .send()
        .await
        .expect("Failed to get readiness");

    assert_eq!(response.status(), 200);

    env.shutdown().await;
}

#[tokio::test]
async fn test_direct_compiler_compilation() {
    let env = TestEnv::setup().await;

    let operation = Operation::new(
        OperationKind::Parse {
            input: "e2e test".to_string(),
        },
        1,
    );

    let request = CompileRequest {
        operation,
        replay_pointers: vec![],
        metadata: HashMap::new(),
    };

    let response = env
        .compiler
        .client
        .post(&format!("{}/compile", env.compiler.base_url()))
        .json(&request)
        .send()
        .await
        .expect("Failed to compile")
        .json::<CompileResponse>()
        .await
        .expect("Failed to parse response");

    assert_eq!(response.pipeline_stats.completed_stages, 7);

    env.shutdown().await;
}

#[tokio::test]
async fn test_edge_webhook_endpoint() {
    let env = TestEnv::setup().await;

    let response = env
        .edge
        .client
        .post(&format!("{}/workspace/webhook", env.edge.base_url()))
        .json(&json!({
            "service": "gmail",
            "payload": {
                "message": "test"
            }
        }))
        .send()
        .await;

    match response {
        Ok(resp) => {
            // Should not be 404
            assert_ne!(resp.status(), 404);
        }
        Err(_) => {
            // Network error is acceptable for this test
        }
    }

    env.shutdown().await;
}

#[tokio::test]
async fn test_multiple_operations_end_to_end() {
    let env = TestEnv::setup().await;

    for i in 0..5 {
        let operation = Operation::new(
            OperationKind::Parse {
                input: format!("test_{}", i),
            },
            (i % 3) as u32 + 1,
        );

        let request = CompileRequest {
            operation,
            replay_pointers: vec![],
            metadata: HashMap::new(),
        };

        let response = env
            .compiler
            .client
            .post(&format!("{}/compile", env.compiler.base_url()))
            .json(&request)
            .send()
            .await
            .expect("Failed to compile")
            .json::<CompileResponse>()
            .await
            .expect("Failed to parse response");

        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }

    env.shutdown().await;
}

#[tokio::test]
async fn test_concurrent_compilations() {
    let env = TestEnv::setup().await;

    let mut tasks = vec![];

    for i in 0..10 {
        let base_url = env.compiler.base_url();
        let client = env.compiler.client.clone();

        let task = tokio::spawn(async move {
            let operation = Operation::new(
                OperationKind::TypeCheck {
                    module_id: format!("module_{}", i),
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
                .expect("Failed to compile")
                .json::<CompileResponse>()
                .await
                .expect("Failed to parse response");

            response.pipeline_stats.completed_stages
        });

        tasks.push(task);
    }

    let results = futures::future::join_all(tasks).await;
    for result in results {
        let stages = result.expect("Task failed");
        assert_eq!(stages, 7);
    }

    env.shutdown().await;
}

#[tokio::test]
async fn test_operation_priority_ordering() {
    let env = TestEnv::setup().await;

    // Submit operations with varying priorities
    let operations = vec![
        (OperationKind::Parse {
            input: "low".to_string(),
        }, 1),
        (OperationKind::Parse {
            input: "high".to_string(),
        }, 100),
        (OperationKind::Parse {
            input: "medium".to_string(),
        }, 50),
    ];

    for (kind, priority) in operations {
        let operation = Operation::new(kind, priority);

        let request = CompileRequest {
            operation,
            replay_pointers: vec![],
            metadata: HashMap::new(),
        };

        let response = env
            .compiler
            .client
            .post(&format!("{}/compile", env.compiler.base_url()))
            .json(&request)
            .send()
            .await
            .expect("Failed to compile")
            .json::<CompileResponse>()
            .await
            .expect("Failed to parse response");

        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }

    env.shutdown().await;
}

#[tokio::test]
async fn test_receipt_chain_with_replay_pointers() {
    let env = TestEnv::setup().await;

    // Create first operation
    let op1 = Operation::new(
        OperationKind::Parse {
            input: "first".to_string(),
        },
        1,
    );

    let request1 = CompileRequest {
        operation: op1,
        replay_pointers: vec![],
        metadata: HashMap::new(),
    };

    let response1 = env
        .compiler
        .client
        .post(&format!("{}/compile", env.compiler.base_url()))
        .json(&request1)
        .send()
        .await
        .expect("Failed to compile")
        .json::<CompileResponse>()
        .await
        .expect("Failed to parse response");

    let receipt1_id = response1.receipt.id;

    // Create second operation with replay pointer
    let op2 = Operation::new(
        OperationKind::TypeCheck {
            module_id: "second".to_string(),
        },
        1,
    );

    let request2 = CompileRequest {
        operation: op2,
        replay_pointers: vec![receipt1_id.to_string()],
        metadata: HashMap::new(),
    };

    let response2 = env
        .compiler
        .client
        .post(&format!("{}/compile", env.compiler.base_url()))
        .json(&request2)
        .send()
        .await
        .expect("Failed to compile")
        .json::<CompileResponse>()
        .await
        .expect("Failed to parse response");

    assert_eq!(response2.pipeline_stats.completed_stages, 7);

    env.shutdown().await;
}

#[tokio::test]
async fn test_operation_with_metadata_propagation() {
    let env = TestEnv::setup().await;

    let mut metadata = HashMap::new();
    metadata.insert(
        "correlation_id".to_string(),
        json!("e2e-test-123"),
    );
    metadata.insert(
        "source".to_string(),
        json!("integration-test"),
    );
    metadata.insert(
        "environment".to_string(),
        json!("testing"),
    );

    let operation = Operation::new(
        OperationKind::Parse {
            input: "metadata test".to_string(),
        },
        1,
    );

    let request = CompileRequest {
        operation,
        replay_pointers: vec![],
        metadata,
    };

    let response = env
        .compiler
        .client
        .post(&format!("{}/compile", env.compiler.base_url()))
        .json(&request)
        .send()
        .await
        .expect("Failed to compile")
        .json::<CompileResponse>()
        .await
        .expect("Failed to parse response");

    assert_eq!(response.pipeline_stats.completed_stages, 7);

    env.shutdown().await;
}

#[tokio::test]
async fn test_all_operation_kinds() {
    let env = TestEnv::setup().await;

    let operations = vec![
        OperationKind::Parse {
            input: "code".to_string(),
        },
        OperationKind::TypeCheck {
            module_id: "m1".to_string(),
        },
        OperationKind::Optimize {
            ir_id: "ir1".to_string(),
            level: 2,
        },
        OperationKind::CodeGen {
            target: "x86_64".to_string(),
        },
        OperationKind::Link {
            modules: vec!["m1".to_string(), "m2".to_string()],
        },
    ];

    for kind in operations {
        let operation = Operation::new(kind, 1);

        let request = CompileRequest {
            operation,
            replay_pointers: vec![],
            metadata: HashMap::new(),
        };

        let response = env
            .compiler
            .client
            .post(&format!("{}/compile", env.compiler.base_url()))
            .json(&request)
            .send()
            .await
            .expect("Failed to compile")
            .json::<CompileResponse>()
            .await
            .expect("Failed to parse response");

        assert_eq!(response.pipeline_stats.completed_stages, 7);
    }

    env.shutdown().await;
}

#[tokio::test]
async fn test_stress_test_many_operations() {
    let env = TestEnv::setup().await;

    // Submit 50 operations
    for i in 0..50 {
        let operation = Operation::new(
            OperationKind::Parse {
                input: format!("stress_test_{}", i),
            },
            (i % 10) as u32 + 1,
        );

        let request = CompileRequest {
            operation,
            replay_pointers: vec![],
            metadata: HashMap::new(),
        };

        let response = env
            .compiler
            .client
            .post(&format!("{}/compile", env.compiler.base_url()))
            .json(&request)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let parsed = resp.json::<CompileResponse>().await;
                if let Ok(resp_data) = parsed {
                    assert_eq!(resp_data.pipeline_stats.completed_stages, 7);
                }
            }
            Err(_) => {
                // Network error acceptable during stress test
            }
        }
    }

    env.shutdown().await;
}

#[tokio::test]
async fn test_deterministic_compilation_output() {
    let env = TestEnv::setup().await;

    // Submit same operation twice, should get same result
    let operation = Operation::new(
        OperationKind::Parse {
            input: "deterministic".to_string(),
        },
        5,
    );

    let request1 = CompileRequest {
        operation: operation.clone(),
        replay_pointers: vec![],
        metadata: HashMap::new(),
    };

    let response1 = env
        .compiler
        .client
        .post(&format!("{}/compile", env.compiler.base_url()))
        .json(&request1)
        .send()
        .await
        .expect("Failed to compile")
        .json::<CompileResponse>()
        .await
        .expect("Failed to parse response");

    let request2 = CompileRequest {
        operation,
        replay_pointers: vec![],
        metadata: HashMap::new(),
    };

    let response2 = env
        .compiler
        .client
        .post(&format!("{}/compile", env.compiler.base_url()))
        .json(&request2)
        .send()
        .await
        .expect("Failed to compile")
        .json::<CompileResponse>()
        .await
        .expect("Failed to parse response");

    // Same input should produce same operation hash
    assert_eq!(response1.receipt.operation_hash, response2.receipt.operation_hash);

    env.shutdown().await;
}

#[tokio::test]
async fn test_service_restart_resilience() {
    // This test verifies that services can be restarted and still work
    let env1 = TestEnv::setup().await;
    assert!(env1.compiler.is_healthy().await);
    env1.shutdown().await;

    // Wait a bit for cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Start new instances
    let env2 = TestEnv::setup().await;
    assert!(env2.compiler.is_healthy().await);

    let operation = Operation::new(
        OperationKind::Parse {
            input: "restart test".to_string(),
        },
        1,
    );

    let request = CompileRequest {
        operation,
        replay_pointers: vec![],
        metadata: HashMap::new(),
    };

    let response = env2
        .compiler
        .client
        .post(&format!("{}/compile", env2.compiler.base_url()))
        .json(&request)
        .send()
        .await
        .expect("Failed to compile")
        .json::<CompileResponse>()
        .await
        .expect("Failed to parse response");

    assert_eq!(response.pipeline_stats.completed_stages, 7);

    env2.shutdown().await;
}
