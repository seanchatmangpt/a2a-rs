#!/usr/bin/env python3
"""
Basic usage example for a2a-construct Python bindings

Demonstrates:
- Creating and managing ontology state
- Adding tasks and messages
- Building cryptographic receipt chains
- Verifying chain integrity
- JSON serialization
"""

from a2a_construct import (
    OntologyState,
    StateBounds,
    Receipt,
    ReceiptChain,
    ConstructError,
)
import json


def main():
    print("=== A2A CONSTRUCT Python Bindings Example ===\n")

    # 1. Create ontology state with custom bounds
    print("1. Creating ontology state...")
    bounds = StateBounds(max_tasks=1000, max_messages_per_task=100, max_agents=50)
    state = OntologyState(bounds=bounds)
    print(f"   State: {state}")
    print(f"   Is empty: {state.is_empty()}\n")

    # 2. Add a task
    print("2. Adding a task...")
    task_json = json.dumps({
        "id": "task-123",
        "contextId": "ctx-abc",
        "status": {
            "state": "pending",
            "reason": None
        },
        "agent": {
            "name": "ExampleAgent",
            "url": "http://localhost:8080",
            "publicKey": None
        }
    })

    try:
        state.put_task_json(task_json)
        print(f"   Task added successfully")
        print(f"   Task count: {state.task_count()}\n")
    except ConstructError as e:
        print(f"   Error: {e}\n")

    # 3. Retrieve the task
    print("3. Retrieving task...")
    retrieved = state.get_task_json("task-123")
    if retrieved:
        task_data = json.loads(retrieved)
        print(f"   Task ID: {task_data['id']}")
        print(f"   Context: {task_data['contextId']}")
        print(f"   Status: {task_data['status']['state']}\n")
    else:
        print("   Task not found\n")

    # 4. Add messages
    print("4. Adding messages to task...")
    messages = [
        {
            "messageId": "msg-1",
            "role": "user",
            "content": [{"type": "text", "text": "What is 2+2?"}]
        },
        {
            "messageId": "msg-2",
            "role": "assistant",
            "content": [{"type": "text", "text": "4"}]
        },
    ]

    for msg in messages:
        state.add_message_json("task-123", json.dumps(msg))
        print(f"   Added message: {msg['messageId']}")

    print(f"   Message count: {state.message_count('task-123')}\n")

    # 5. Get state statistics
    print("5. State statistics...")
    stats = state.stats()
    print(f"   {stats}\n")

    # 6. Create receipts
    print("6. Creating cryptographic receipts...")
    receipt1 = Receipt.new(
        b"User query: What is 2+2?",
        b"Agent response: 4",
        b"state: query_count += 1"
    )
    print(f"   {receipt1}")
    print(f"   Hash: {receipt1.receipt_hash()[:16]}...")
    print(f"   Timestamp: {receipt1.timestamp()}\n")

    # 7. Build receipt chain
    print("7. Building receipt chain...")
    chain = ReceiptChain.new()

    # Add transitions
    transitions = [
        (b"obs1: user message", b"act1: process query", b"delta1: update state"),
        (b"obs2: assistant response", b"act2: send message", b"delta2: add message"),
        (b"obs3: task completion", b"act3: finalize task", b"delta3: mark complete"),
    ]

    for i, (obs, act, delta) in enumerate(transitions, 1):
        receipt = chain.add_transition(obs, act, delta)
        print(f"   Step {i}: seq={receipt.sequence()}, hash={receipt.receipt_hash()[:16]}...")

    print(f"   Chain length: {chain.length()}\n")

    # 8. Verify chain integrity
    print("8. Verifying chain integrity...")
    try:
        if chain.verify_integrity():
            print("   ✓ Chain integrity verified\n")
    except ConstructError as e:
        print(f"   ✗ Verification failed: {e}\n")

    # 9. Chain iteration
    print("9. Iterating over chain...")
    for i in range(chain.length()):
        receipt = chain.get(i)
        if receipt:
            print(f"   Receipt {i}: {receipt}")
    print()

    # 10. Latest receipt
    print("10. Getting latest receipt...")
    latest = chain.latest()
    if latest:
        print(f"    {latest}")
        print(f"    Previous hash: {latest.previous_hash()[:16] if latest.previous_hash() else 'None'}...\n")

    # 11. JSON serialization
    print("11. JSON serialization...")

    # Export state
    state_json = state.to_json()
    print(f"    State JSON length: {len(state_json)} chars")

    # Export chain
    chain_json = chain.to_json()
    print(f"    Chain JSON length: {len(chain_json)} chars")

    # Round-trip test
    restored_state = OntologyState.from_json(state_json)
    print(f"    Restored state: {restored_state}")

    restored_chain = ReceiptChain.from_json(chain_json)
    print(f"    Restored chain: {restored_chain}")

    # Verify restored chain
    try:
        if restored_chain.verify_integrity():
            print("    ✓ Restored chain integrity verified\n")
    except ConstructError as e:
        print(f"    ✗ Restored chain verification failed: {e}\n")

    # 12. Clear state
    print("12. Clearing state...")
    state.clear()
    print(f"    Is empty: {state.is_empty()}")
    print(f"    Task count: {state.task_count()}\n")

    print("=== Example Complete ===")


if __name__ == "__main__":
    main()
