use serde::Serialize;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Serialize, Error)]
pub struct TaskExecutorError {
    pub serialized_err: String,
}

impl fmt::Display for TaskExecutorError {
    /// Formats the TaskExecutorError for display
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.serialized_err)
    }
}
