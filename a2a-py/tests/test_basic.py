"""
Basic tests for a2a-construct Python bindings
"""

import pytest
import json
from a2a_construct import (
    OntologyState,
    StateBounds,
    StateStats,
    Receipt,
    ReceiptChain,
    ConstructError,
)


class TestStateBounds:
    """Test StateBounds configuration"""

    def test_default_bounds(self):
        bounds = StateBounds()
        assert bounds.max_tasks == 10000
        assert bounds.max_messages_per_task == 1000
        assert bounds.max_agents == 1000

    def test_custom_bounds(self):
        bounds = StateBounds(max_tasks=100, max_messages_per_task=50, max_agents=10)
        assert bounds.max_tasks == 100
        assert bounds.max_messages_per_task == 50
        assert bounds.max_agents == 10

    def test_repr(self):
        bounds = StateBounds(max_tasks=100, max_messages_per_task=50, max_agents=10)
        repr_str = repr(bounds)
        assert "100" in repr_str
        assert "50" in repr_str
        assert "10" in repr_str


class TestOntologyState:
    """Test OntologyState management"""

    def test_new_state_is_empty(self):
        state = OntologyState()
        assert state.is_empty()
        assert state.task_count() == 0
        assert state.agent_count() == 0

    def test_put_and_get_task(self):
        state = OntologyState()
        task_json = json.dumps({
            "id": "task-1",
            "contextId": "ctx-1",
            "status": {"state": "pending", "reason": None},
            "agent": {"name": "TestAgent", "url": "http://localhost", "publicKey": None}
        })

        state.put_task_json(task_json)
        assert state.task_count() == 1

        retrieved = state.get_task_json("task-1")
        assert retrieved is not None
        task_data = json.loads(retrieved)
        assert task_data["id"] == "task-1"
        assert task_data["contextId"] == "ctx-1"

    def test_remove_task(self):
        state = OntologyState()
        task_json = json.dumps({
            "id": "task-1",
            "contextId": "ctx-1",
            "status": {"state": "pending", "reason": None},
            "agent": {"name": "TestAgent", "url": "http://localhost", "publicKey": None}
        })

        state.put_task_json(task_json)
        assert state.task_count() == 1

        removed = state.remove_task("task-1")
        assert removed is not None
        assert state.task_count() == 0
        assert state.is_empty()

    def test_add_and_get_messages(self):
        state = OntologyState()

        # First add a task
        task_json = json.dumps({
            "id": "task-1",
            "contextId": "ctx-1",
            "status": {"state": "pending", "reason": None},
            "agent": {"name": "TestAgent", "url": "http://localhost", "publicKey": None}
        })
        state.put_task_json(task_json)

        # Add messages
        msg1 = json.dumps({
            "messageId": "msg-1",
            "role": "user",
            "content": [{"type": "text", "text": "Hello"}]
        })
        msg2 = json.dumps({
            "messageId": "msg-2",
            "role": "assistant",
            "content": [{"type": "text", "text": "Hi there"}]
        })

        state.add_message_json("task-1", msg1)
        state.add_message_json("task-1", msg2)

        assert state.message_count("task-1") == 2

        messages_json = state.get_messages_json("task-1")
        assert messages_json is not None
        messages = json.loads(messages_json)
        assert len(messages) == 2
        assert messages[0]["messageId"] == "msg-1"
        assert messages[1]["messageId"] == "msg-2"

    def test_stats(self):
        state = OntologyState()

        # Add task
        task_json = json.dumps({
            "id": "task-1",
            "contextId": "ctx-1",
            "status": {"state": "pending", "reason": None},
            "agent": {"name": "TestAgent", "url": "http://localhost", "publicKey": None}
        })
        state.put_task_json(task_json)

        # Add message
        msg_json = json.dumps({
            "messageId": "msg-1",
            "role": "user",
            "content": [{"type": "text", "text": "Test"}]
        })
        state.add_message_json("task-1", msg_json)

        stats = state.stats()
        assert stats.task_count == 1
        assert stats.total_messages == 1

    def test_clear(self):
        state = OntologyState()
        task_json = json.dumps({
            "id": "task-1",
            "contextId": "ctx-1",
            "status": {"state": "pending", "reason": None},
            "agent": {"name": "TestAgent", "url": "http://localhost", "publicKey": None}
        })
        state.put_task_json(task_json)

        assert not state.is_empty()
        state.clear()
        assert state.is_empty()

    def test_json_serialization(self):
        state = OntologyState()
        task_json = json.dumps({
            "id": "task-1",
            "contextId": "ctx-1",
            "status": {"state": "pending", "reason": None},
            "agent": {"name": "TestAgent", "url": "http://localhost", "publicKey": None}
        })
        state.put_task_json(task_json)

        # Export
        exported = state.to_json()
        assert len(exported) > 0

        # Import
        restored = OntologyState.from_json(exported)
        assert restored.task_count() == 1
        assert restored.get_task_json("task-1") is not None


class TestReceipt:
    """Test Receipt creation and verification"""

    def test_create_receipt(self):
        receipt = Receipt.new(b"observation", b"action", b"delta")
        assert receipt.sequence() == 0
        assert len(receipt.observation_hash()) > 0
        assert len(receipt.action_hash()) > 0
        assert len(receipt.delta_hash()) > 0
        assert len(receipt.receipt_hash()) > 0
        assert receipt.previous_hash() is None

    def test_verify_hashes(self):
        receipt = Receipt.new(b"observation", b"action", b"delta")
        # Should not raise
        receipt.verify_hashes()

    def test_json_serialization(self):
        receipt = Receipt.new(b"observation", b"action", b"delta")

        # Export
        exported = receipt.to_json()
        assert len(exported) > 0

        # Import
        restored = Receipt.from_json(exported)
        assert restored.receipt_hash() == receipt.receipt_hash()
        assert restored.observation_hash() == receipt.observation_hash()


class TestReceiptChain:
    """Test ReceiptChain integrity"""

    def test_create_empty_chain(self):
        chain = ReceiptChain()
        assert chain.is_empty()
        assert chain.length() == 0
        assert len(chain) == 0

    def test_add_transitions(self):
        chain = ReceiptChain()

        receipt1 = chain.add_transition(b"obs1", b"act1", b"delta1")
        assert receipt1.sequence() == 0
        assert receipt1.previous_hash() is None

        receipt2 = chain.add_transition(b"obs2", b"act2", b"delta2")
        assert receipt2.sequence() == 1
        assert receipt2.previous_hash() == receipt1.receipt_hash()

        assert chain.length() == 2
        assert not chain.is_empty()

    def test_verify_integrity(self):
        chain = ReceiptChain()
        chain.add_transition(b"obs1", b"act1", b"delta1")
        chain.add_transition(b"obs2", b"act2", b"delta2")
        chain.add_transition(b"obs3", b"act3", b"delta3")

        # Should not raise
        assert chain.verify_integrity()

    def test_get_receipt(self):
        chain = ReceiptChain()
        chain.add_transition(b"obs1", b"act1", b"delta1")
        chain.add_transition(b"obs2", b"act2", b"delta2")

        receipt = chain.get(0)
        assert receipt is not None
        assert receipt.sequence() == 0

        receipt = chain.get(1)
        assert receipt is not None
        assert receipt.sequence() == 1

        receipt = chain.get(999)
        assert receipt is None

    def test_latest_receipt(self):
        chain = ReceiptChain()
        assert chain.latest() is None

        chain.add_transition(b"obs1", b"act1", b"delta1")
        chain.add_transition(b"obs2", b"act2", b"delta2")

        latest = chain.latest()
        assert latest is not None
        assert latest.sequence() == 1

    def test_json_serialization(self):
        chain = ReceiptChain()
        chain.add_transition(b"obs1", b"act1", b"delta1")
        chain.add_transition(b"obs2", b"act2", b"delta2")

        # Export
        exported = chain.to_json()
        assert len(exported) > 0

        # Import
        restored = ReceiptChain.from_json(exported)
        assert restored.length() == 2
        assert restored.verify_integrity()

        # Check hashes match
        for i in range(chain.length()):
            original = chain.get(i)
            restored_receipt = restored.get(i)
            assert original.receipt_hash() == restored_receipt.receipt_hash()


class TestErrors:
    """Test error handling"""

    def test_invalid_task_json(self):
        state = OntologyState()
        with pytest.raises(ConstructError):
            state.put_task_json("invalid json")

    def test_add_message_to_nonexistent_task(self):
        state = OntologyState()
        msg_json = json.dumps({
            "messageId": "msg-1",
            "role": "user",
            "content": [{"type": "text", "text": "Test"}]
        })

        with pytest.raises(ConstructError):
            state.add_message_json("nonexistent-task", msg_json)

    def test_invalid_receipt_json(self):
        with pytest.raises(ConstructError):
            Receipt.from_json("invalid json")

    def test_invalid_chain_json(self):
        with pytest.raises(ConstructError):
            ReceiptChain.from_json("invalid json")
