"""
A2A CONSTRUCT - Python bindings for deterministic agent runtime

This package provides Python access to the CONSTRUCT layer of the A2A protocol,
enabling ontology state management, cryptographic receipts, and deterministic execution.

Example:
    >>> from a2a_construct import OntologyState, ReceiptChain
    >>> state = OntologyState.new()
    >>> state.task_count()
    0
    >>> chain = ReceiptChain.new()
    >>> chain.add_transition(b"obs", b"act", b"delta")
    >>> chain.verify_integrity()
    True
"""

from .a2a_construct import (
    OntologyState,
    StateBounds,
    StateStats,
    Receipt,
    ReceiptChain,
    ConstructError,
    __version__,
)

__all__ = [
    "OntologyState",
    "StateBounds",
    "StateStats",
    "Receipt",
    "ReceiptChain",
    "ConstructError",
    "__version__",
]
