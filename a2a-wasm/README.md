# a2a-wasm

WebAssembly bindings for the A2A construct runtime, enabling browser-compatible execution of A2A protocol stations.

## Features

- **Browser-compatible**: Pure WASM with no I/O dependencies
- **Deterministic execution**: Station-based packet processing
- **Stateful or stateless**: Maintain ontology state across calls or use fresh state
- **Full A2A v0.3.0 support**: All core protocol methods

## Building

```bash
# Install wasm-pack (if not already installed)
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build for web (bundler)
wasm-pack build --target web

# Build for Node.js
wasm-pack build --target nodejs

# Build for bundlers (webpack, etc.)
wasm-pack build --target bundler
```

## Usage

### JavaScript/TypeScript

```javascript
import init, { execute_station } from './a2a_wasm.js';

// Initialize the WASM module
await init();

// Create a request packet
const request = {
  jsonrpc: "2.0",
  id: "req-1",
  method: "message/send",
  params: {
    message: {
      role: "user",
      parts: [{ text: "Hello, agent!" }]
    }
  }
};

// Execute with state management
const result = JSON.parse(execute_station(
  JSON.stringify(request),
  "{}" // Empty initial state
));

console.log('Response:', result.response);
console.log('Updated state:', result.state);
console.log('Success:', result.success);

// For next call, pass the updated state
const nextResult = execute_station(
  JSON.stringify(nextRequest),
  JSON.stringify(result.state)
);
```

### Stateless Execution

For simple one-off operations:

```javascript
import { execute_station_stateless } from './a2a_wasm.js';

const request = {
  jsonrpc: "2.0",
  id: "req-1",
  method: "tasks/list",
  params: {}
};

const response = JSON.parse(execute_station_stateless(
  JSON.stringify(request)
));
```

### Utility Functions

```javascript
import { version, is_method_supported, list_supported_methods } from './a2a_wasm.js';

// Check version
console.log('WASM version:', version());

// Check method support
if (is_method_supported('message/send')) {
  console.log('message/send is supported');
}

// List all supported methods
const methods = JSON.parse(list_supported_methods());
console.log('Supported methods:', methods);
```

## Supported Methods

- `message/send` - Send a message
- `message/stream` - Send a streaming message
- `tasks/get` - Get a task by ID
- `tasks/cancel` - Cancel a task
- `tasks/list` - List tasks with filtering
- `tasks/resubscribe` - Resubscribe to a streaming task
- `tasks/pushNotificationConfig/set` - Set push notification config
- `tasks/pushNotificationConfig/get` - Get push notification config
- `tasks/pushNotificationConfig/list` - List push notification configs
- `tasks/pushNotificationConfig/delete` - Delete push notification config
- `agent/getExtendedCard` - Get agent extended card
- `agent/getAuthenticatedExtendedCard` - Get authenticated agent card

## State Management

The WASM module maintains an `OntologyState` that tracks:
- Tasks (with messages and metadata)
- Agents
- Push notification configurations

You can either:
1. **Manage state externally**: Pass serialized state between calls via `execute_station()`
2. **Use stateless mode**: Let each call start with a fresh state via `execute_station_stateless()`

## Error Handling

All errors are returned as JSON-RPC error responses:

```json
{
  "jsonrpc": "2.0",
  "id": "req-1",
  "error": {
    "code": -32001,
    "message": "Task not found: task-123",
    "data": null
  }
}
```

Common error codes:
- `-32700`: Parse error
- `-32602`: Invalid params
- `-32601`: Method not found
- `-32001`: Task not found
- `-32002`: Invalid state transition
- `-32603`: Internal error

## Performance

The WASM module is optimized for size and performance:
- Release builds use `opt-level = "z"` (optimize for size)
- LTO (Link Time Optimization) enabled
- Optional `wee_alloc` for smaller binary size

Typical bundle size: ~200KB gzipped

## Architecture

The WASM bindings are a thin wrapper around the construct runtime:

```
JavaScript → wasm-bindgen → StationRegistry → Station → OntologyState
```

All execution is deterministic and pure computation - no network I/O, no file system access, no async runtime.

## License

MIT
