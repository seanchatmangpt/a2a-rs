//! Event types for streaming and notifications

pub mod task_events;

pub use task_events::{TaskArtifactUpdateEvent, TaskStatusUpdateEvent};

#[cfg(test)]
mod tests_deny_unknown_fields;
