# Adapter Implementation Unit Tests

This directory contains comprehensive unit tests for all adapter implementations in the a2a-rs workspace.

## Test Files

### 1. HTTP Client Tests (`http_client_test.rs`)
**Lines:** 889
**Tests:** 31 tests

Tests the HTTP client adapter implementation with a focus on:
- Request/response handling
- JSON-RPC protocol compliance
- Error handling (404, 500, timeouts)
- Concurrent request handling
- Task operations (send, get, cancel, list)
- Push notification configuration
- Streaming unsupported operation errors
- Request tracking and validation
- Session ID handling
- Authentication with bearer tokens

**Key Test Areas:**
- Raw request sending with mock responses
- Success and failure scenarios
- Task CRUD operations
- Error conversion to JSON-RPC format
- Request payload serialization
- Response parsing

### 2. HTTP Server Tests (`http_server_test.rs`)
**Lines:** 609
**Tests:** 30 tests

Tests the HTTP server adapter with mock request processors:
- Request processing and routing
- JSON-RPC validation
- Error response generation
- Agent card and skills retrieval
- Task storage and retrieval
- Concurrent request handling
- Large payload handling
- Special character handling
- Validation error responses
- Custom error injection

**Key Test Areas:**
- Request processor contract
- Agent info provider
- Task storage operations
- Error-to-JSON-RPC conversion
- Protocol version validation
- Multi-request handling
- Edge cases (empty params, null IDs, whitespace)

### 3. WebSocket Client Tests (`websocket_client_test.rs`)
**Lines:** 565
**Tests:** 39 tests

Tests the WebSocket client adapter with focus on:
- Connection state management
- Session state tracking
- Reconnection configuration and backoff logic
- Heartbeat configuration
- Request queue configuration
- URL parsing and validation
- Timeout scenarios
- Concurrent operations
- Unsupported streaming operations

**Key Test Areas:**
- Client creation and configuration
- Connection status transitions
- Reconnect backoff calculation with exponential backoff
- Jitter factor for backoff randomization
- Session expiration detection
- Configuration builders (reconnect, heartbeat, queue)
- Concurrent request handling
- Subscription operations

### 4. WebSocket Server Tests (`websocket_server_test.rs`)
**Lines:** 759
**Tests:** 30 tests

Tests the WebSocket server adapter with mock streaming handlers:
- Streaming handler contract
- Status and artifact subscriber management
- Subscriber notification delivery
- Multiple task subscriptions
- Subscriber isolation
- Request processing
- Agent info provision
- Concurrent operations

**Key Test Areas:**
- Adding/removing status subscribers
- Adding/removing artifact subscribers
- Multi-subscriber scenarios
- Subscriber notification events
- Request processor implementation
- Agent card structure validation
- Concurrent subscriber operations
- Handler cloning behavior

### 5. Authentication Adapter Tests (`auth_adapter_test.rs`)
**Lines:** 668
**Tests:** 49 tests

Tests all authentication adapter implementations:
- Bearer token authenticator
- API key authenticator (header, query, cookie)
- No-op authenticator
- Context extractors
- Principal management
- Security scheme generation
- Token validation
- Scheme validation
- Metadata handling

**Key Test Areas:**
- Valid token/key authentication
- Invalid token/key rejection
- Wrong scheme detection
- Security scheme structure
- Bearer token extraction from headers
- API key extraction from headers/query/cookies
- Case sensitivity handling
- Principal attribute management
- Empty token/key scenarios
- Authenticator cloning

## Test Organization

All tests follow the hexagonal architecture pattern:
- **Mock implementations** of port traits for testing
- **Independent testing** of adapter business logic
- **Edge case coverage** for error paths
- **Concurrency testing** for thread safety
- **Property-based testing** approach where applicable

## Running Tests

Tests are organized as standalone integration tests. To run all adapter tests:

```bash
# Run all adapter tests
cargo test --package a2a-rs --test unit::adapter

# Run specific test file
cargo test --package a2a-rs --test unit::adapter::http_client_test
cargo test --package a2a-rs --test unit::adapter::http_server_test
cargo test --package a2a-rs --test unit::adapter::websocket_client_test
cargo test --package a2a-rs --test unit::adapter::websocket_server_test
cargo test --package a2a-rs --test unit::adapter::auth_adapter_test
```

## Coverage

The test suite covers:
- **179 total tests** across 5 test files
- **~3,500 lines** of test code
- All adapter implementations in `a2a-rs/src/adapter/`
- Transport adapters (HTTP, WebSocket)
- Authentication adapters
- Error handling paths
- Edge cases and boundary conditions

## Dependencies

Tests use:
- `tokio` for async runtime
- `async_trait` for async trait testing
- `serde_json` for JSON serialization testing
- `chrono` for timestamp handling
- `uuid` for unique ID generation
- Mock implementations of all port traits

## Design Principles

1. **Mock First:** Each test creates mock implementations of required ports
2. **Isolation:** Tests are independent and can run in parallel
3. **Clarity:** Test names clearly describe what is being tested
4. **Comprehensive:** Both success and failure paths are tested
5. **Realistic:** Tests use realistic domain types and payloads
