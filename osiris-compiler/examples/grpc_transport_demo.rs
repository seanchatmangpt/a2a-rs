//! gRPC transport demonstration example.
//!
//! Shows how to use the gRPC Transport implementation with:
//! - Single request-response operations
//! - Client streaming (multiple operations)
//! - Server streaming (receipt subscriptions)
//! - Bidirectional streaming (full duplex)
//! - Statistics and backpressure management

#[cfg(feature = "grpc")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use futures::stream;
    use osiris_compiler::prelude::*;

    println!("=== Osiris gRPC Transport Demo ===\n");

    // Create transport with default configuration
    let config = TransportConfig::default();
    let transport = GrpcTransport::new(config);

    println!("Transport initialized");
    println!("  Server: {}", transport.config().server_address);
    println!(
        "  Max message size: {} bytes",
        transport.config().max_message_size_bytes
    );
    println!(
        "  Backpressure limit: {}\n",
        transport.config().backpressure_limit
    );

    // Example 1: Single request-response operation
    println!("--- Example 1: Single Operation ---");
    {
        let operation = Operation::new(
            OperationKind::Parse {
                input: "let x = 42;".to_string(),
            },
            10,
        );

        println!("Sending operation: {:?}", operation.kind);
        let receipt = transport.send_operation(operation.clone()).await?;

        println!("Received receipt:");
        println!("  Receipt ID: {}", receipt.id);
        println!("  Operation ID: {}", receipt.operation_id);
        println!("  Status: Success\n");
    }

    // Example 2: Client streaming
    println!("--- Example 2: Client Streaming ---");
    {
        let ops = vec![
            Operation::new(
                OperationKind::Parse {
                    input: "parse1".into(),
                },
                5,
            ),
            Operation::new(
                OperationKind::TypeCheck {
                    module_id: "module1".into(),
                },
                8,
            ),
            Operation::new(
                OperationKind::Optimize {
                    ir_id: "ir1".into(),
                    level: 2,
                },
                7,
            ),
        ];

        println!("Sending {} operations via client streaming...", ops.len());

        let stream = stream::iter(ops.clone().into_iter().map(Ok));
        let mut receipt_stream = transport.client_streaming(Box::pin(stream)).await?;

        let mut count = 0;
        while let Some(result) = futures::StreamExt::next(&mut receipt_stream).await {
            match result {
                Ok(receipt) => {
                    count += 1;
                    println!(
                        "  Received receipt {} for operation {}",
                        count, receipt.operation_id
                    );
                }
                Err(e) => eprintln!("  Error receiving receipt: {}", e),
            }
        }
        println!("Received {} receipts\n", count);
    }

    // Example 3: Server streaming
    println!("--- Example 3: Server Streaming ---");
    {
        println!("Subscribing to receipt stream from server...");

        let mut receipt_stream = transport
            .server_streaming(Some("filter:Parse".to_string()))
            .await?;

        let mut count = 0;
        while let Some(result) = futures::StreamExt::next(&mut receipt_stream).await {
            match result {
                Ok(receipt) => {
                    count += 1;
                    println!("  Received server-pushed receipt {}", count);
                    if count >= 3 {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("  Error: {}", e);
                    break;
                }
            }
        }
        println!("Received {} server-streamed receipts\n", count);
    }

    // Example 4: Bidirectional streaming
    println!("--- Example 4: Bidirectional Streaming ---");
    {
        let ops = vec![
            Operation::new(
                OperationKind::CodeGen {
                    target: "wasm".into(),
                },
                9,
            ),
            Operation::new(
                OperationKind::Link {
                    modules: vec!["mod1".into(), "mod2".into()],
                },
                6,
            ),
        ];

        println!(
            "Starting full-duplex streaming with {} operations...",
            ops.len()
        );

        let stream = stream::iter(ops.into_iter().map(Ok));
        let mut receipt_stream = transport.bidirectional_streaming(Box::pin(stream)).await?;

        let mut count = 0;
        while let Some(result) = futures::StreamExt::next(&mut receipt_stream).await {
            match result {
                Ok(receipt) => {
                    count += 1;
                    println!(
                        "  Received async receipt {} for operation {}",
                        count, receipt.operation_id
                    );
                }
                Err(e) => {
                    eprintln!("  Error: {}", e);
                    break;
                }
            }
        }
        println!("Received {} bidirectional receipts\n", count);
    }

    // Example 5: Statistics
    println!("--- Example 5: Statistics ---");
    {
        let stats = transport.get_stats();
        println!("Transport Statistics:");
        println!("  Operations sent: {}", stats.operations_sent);
        println!("  Receipts received: {}", stats.receipts_received);
        println!("  Bytes sent: {}", stats.bytes_sent);
        println!("  Bytes received: {}", stats.bytes_received);
        println!("  Average latency: {:.2} ms", stats.avg_latency_ms);
        println!("  Total duration: {} ms\n", stats.total_duration_ms);
    }

    // Example 6: Configuration builder
    println!("--- Example 6: Custom Configuration ---");
    {
        let custom_config = TransportConfig::builder()
            .server_address("api.example.com:50052".to_string())
            .connection_timeout_secs(60)
            .max_message_size_bytes(50 * 1024 * 1024) // 50 MB
            .enable_compression(true)
            .backpressure_limit(5000)
            .max_retries(5)
            .retry_delay_ms(200)
            .build();

        let custom_transport = GrpcTransport::new(custom_config);
        println!("Custom transport created:");
        println!("  Server: {}", custom_transport.config().server_address);
        println!(
            "  Timeout: {} seconds",
            custom_transport.config().connection_timeout_secs
        );
        println!(
            "  Max message: {} MB",
            custom_transport.config().max_message_size_bytes / (1024 * 1024)
        );
        println!(
            "  Compression: {}",
            custom_transport.config().enable_compression
        );
        println!();
    }

    // Example 7: Connection management
    println!("--- Example 7: Connection Management ---");
    {
        println!(
            "Connection status: {}",
            if transport.is_connected().await {
                "connected"
            } else {
                "disconnected"
            }
        );

        transport.close().await?;
        println!("Connection closed");
        println!(
            "Connection status: {}",
            if transport.is_connected().await {
                "connected"
            } else {
                "disconnected"
            }
        );
        println!();
    }

    println!("=== Demo Complete ===");
    Ok(())
}

#[cfg(not(feature = "grpc"))]
fn main() {
    println!("This example requires the 'grpc' feature to be enabled.");
    println!("Run with: cargo run --example grpc_transport_demo --features grpc");
}
