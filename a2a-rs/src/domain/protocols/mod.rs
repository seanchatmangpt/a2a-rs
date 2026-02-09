//! Protocol-specific types and implementations

pub mod json_rpc;

pub use json_rpc::{
    JSONRPCError, JSONRPCMessage, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse,
};

#[cfg(test)]
mod tests_deny_unknown_fields;
