# A2A Load Testing Tool

A comprehensive load testing tool for the A2A protocol that generates concurrent typed packet streams and measures performance metrics.

## Features

- **Concurrent packet generation** using tokio tasks
- **Multiple packet types**: Message, Task, or Mixed
- **Configurable throughput**: Test with 1k, 10k, 100k ops/sec
- **Detailed metrics**: Throughput, latency (p50, p95, p99, max)
- **CSV output**: Results exported for further analysis
- **Real-time reporting**: Periodic metrics during test execution

## Usage

```bash
# Basic usage - 1000 ops/sec for 60 seconds
cargo run -p a2a-loadtest

# High throughput test - 10,000 ops/sec for 30 seconds
cargo run -p a2a-loadtest -- --ops-per-sec 10000 --duration 30

# Ultra-high load - 100,000 ops/sec with 50 workers
cargo run -p a2a-loadtest -- --ops-per-sec 100000 --workers 50 --duration 10

# Generate task packets instead of messages
cargo run -p a2a-loadtest -- --packet-type task --ops-per-sec 5000

# Mixed packet types with custom output file
cargo run -p a2a-loadtest -- \
  --packet-type mixed \
  --ops-per-sec 10000 \
  --duration 120 \
  --workers 20 \
  --output results_mixed_10k.csv \
  --report-interval 10

# With server endpoint (when available)
cargo run -p a2a-loadtest -- \
  --server http://localhost:8080 \
  --ops-per-sec 1000

# Verbose logging
cargo run -p a2a-loadtest -- --verbose --ops-per-sec 5000
```

## Command-Line Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--ops-per-sec` | `-o` | 1000 | Target operations per second |
| `--duration` | `-d` | 60 | Test duration in seconds |
| `--workers` | `-w` | 10 | Number of concurrent workers |
| `--packet-type` | `-p` | message | Packet type: message, task, or mixed |
| `--output` | `-o` | loadtest_results.csv | Output CSV file path |
| `--report-interval` | `-r` | 5 | Reporting interval in seconds |
| `--server` | `-s` | None | Server endpoint (optional) |
| `--verbose` | `-v` | false | Enable verbose logging |

## Output Format

The tool generates a CSV file with the following columns:

- `timestamp`: ISO 8601 timestamp
- `elapsed_secs`: Seconds since test start
- `total_ops`: Cumulative operations completed
- `throughput_ops_per_sec`: Current throughput
- `latency_p50_us`: 50th percentile latency (microseconds)
- `latency_p95_us`: 95th percentile latency (microseconds)
- `latency_p99_us`: 99th percentile latency (microseconds)
- `latency_max_us`: Maximum latency (microseconds)
- `errors`: Cumulative error count

## Example Output

```
A2A Load Test Configuration:
  Target ops/sec: 10000
  Duration: 60s
  Workers: 20
  Packet type: Mixed
  Output file: loadtest_results.csv
  Report interval: 5s

Elapsed: 5.0s | Ops: 50234 | Throughput: 10047 ops/s | Latency p50: 42μs, p99: 156μs | Errors: 0
Elapsed: 10.0s | Ops: 100108 | Throughput: 10011 ops/s | Latency p50: 41μs, p99: 158μs | Errors: 0
...

=== Final Results ===
Total operations: 600654
Duration: 60.00s
Throughput: 10010.90 ops/s
Latency p50: 42μs (0.04ms)
Latency p95: 134μs (0.13ms)
Latency p99: 157μs (0.16ms)
Latency max: 2341μs (2.34ms)
Errors: 0
Results written to: loadtest_results.csv
```

## Architecture

The tool uses a multi-component architecture:

1. **Rate Limiter**: Distributes work at the target ops/sec rate
2. **Worker Pool**: Concurrent tokio tasks that generate and serialize packets
3. **Metrics Collector**: Aggregates results using HDR histograms for accurate percentiles
4. **CSV Writer**: Exports metrics for analysis

## Packet Generation

- **Message packets**: Random text content with user/agent roles
- **Task packets**: Various task states (submitted, working, completed, etc.)
- **Mixed mode**: Random selection between message and task packets

All packets are fully typed using the a2a-rs domain types and serialized to JSON.

## Performance Considerations

- Uses `hdrhistogram` for accurate latency percentiles with minimal overhead
- Tokio channels for efficient async communication
- Lock-free metrics collection
- Configurable worker pool size to match CPU cores
- Round-robin work distribution for load balancing

## Testing Scenarios

### 1. Baseline (1K ops/sec)
```bash
cargo run -p a2a-loadtest -- --ops-per-sec 1000 --duration 60
```

### 2. Medium Load (10K ops/sec)
```bash
cargo run -p a2a-loadtest -- --ops-per-sec 10000 --duration 60 --workers 20
```

### 3. High Load (100K ops/sec)
```bash
cargo run -p a2a-loadtest -- --ops-per-sec 100000 --duration 30 --workers 50
```

## Requirements

- Rust 1.85+
- Tokio async runtime
- a2a-rs library with http-client feature
