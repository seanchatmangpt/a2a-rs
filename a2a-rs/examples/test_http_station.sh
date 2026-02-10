#!/bin/bash
# Test script for http_station example
# Usage: ./test_http_station.sh

set -e

BASE_URL="http://localhost:8080"

echo "=== HTTP Station Test Script ==="
echo ""

# Test 1: Health check
echo "1. Testing health endpoint..."
curl -s "${BASE_URL}/health"
echo ""
echo ""

# Test 2: Get stats (should show zero tasks initially)
echo "2. Getting initial stats..."
curl -s "${BASE_URL}/stats" | jq '.'
echo ""

# Test 3: Send a message (creates a task)
echo "3. Sending a message (creates task)..."
TASK_RESPONSE=$(curl -s -X POST "${BASE_URL}/jsonrpc" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "message/send",
    "params": {
      "message": {
        "role": "user",
        "parts": [{"text": "Hello from test script!"}],
        "messageId": "msg-test-001"
      }
    }
  }')

echo "$TASK_RESPONSE" | jq '.'
echo ""

# Extract task ID from response
TASK_ID=$(echo "$TASK_RESPONSE" | jq -r '.result.id')
echo "Created task ID: $TASK_ID"
echo ""

# Test 4: Get the task
echo "4. Retrieving task..."
curl -s -X POST "${BASE_URL}/jsonrpc" \
  -H "Content-Type: application/json" \
  -d "{
    \"jsonrpc\": \"2.0\",
    \"id\": \"2\",
    \"method\": \"tasks/get\",
    \"params\": {
      \"id\": \"$TASK_ID\"
    }
  }" | jq '.'
echo ""

# Test 5: List tasks
echo "5. Listing all tasks..."
curl -s -X POST "${BASE_URL}/jsonrpc" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": "3",
    "method": "tasks/list",
    "params": {}
  }' | jq '.'
echo ""

# Test 6: Send another message to the same task
echo "6. Sending another message to the task..."
curl -s -X POST "${BASE_URL}/jsonrpc" \
  -H "Content-Type: application/json" \
  -d "{
    \"jsonrpc\": \"2.0\",
    \"id\": \"4\",
    \"method\": \"message/send\",
    \"params\": {
      \"message\": {
        \"role\": \"user\",
        \"parts\": [{\"text\": \"Follow-up message\"}],
        \"messageId\": \"msg-test-002\",
        \"taskId\": \"$TASK_ID\"
      }
    }
  }" | jq '.'
echo ""

# Test 7: Cancel the task
echo "7. Canceling task..."
curl -s -X POST "${BASE_URL}/jsonrpc" \
  -H "Content-Type: application/json" \
  -d "{
    \"jsonrpc\": \"2.0\",
    \"id\": \"5\",
    \"method\": \"tasks/cancel\",
    \"params\": {
      \"id\": \"$TASK_ID\"
    }
  }" | jq '.'
echo ""

# Test 8: Final stats
echo "8. Final stats..."
curl -s "${BASE_URL}/stats" | jq '.'
echo ""

# Test 9: Error handling - try to get non-existent task
echo "9. Testing error handling (non-existent task)..."
curl -s -X POST "${BASE_URL}/jsonrpc" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": "6",
    "method": "tasks/get",
    "params": {
      "id": "non-existent-task"
    }
  }' | jq '.'
echo ""

# Test 10: Invalid method
echo "10. Testing invalid method..."
curl -s -X POST "${BASE_URL}/jsonrpc" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": "7",
    "method": "invalid/method",
    "params": {}
  }' | jq '.'
echo ""

echo "=== All tests completed ==="
