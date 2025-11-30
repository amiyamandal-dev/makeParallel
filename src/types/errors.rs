use pyo3::prelude::*;
use pyo3::exceptions::PyException;
use thiserror::Error;

/// Custom error types for makeParallel
#[derive(Error, Debug, Clone)]
pub enum MakeParallelError {
    #[error("Task '{task_id}' was cancelled: {reason}")]
    TaskCancelled { task_id: String, reason: String },
    
    #[error("Task '{task_id}' timed out after {timeout_secs}s")]
    TaskTimeout { task_id: String, timeout_secs: f64 },
    
    #[error("Cannot start new tasks: shutdown in progress")]
    ShutdownInProgress,
    
    #[error("Memory limit exceeded: {limit_percent}% limit reached")]
    MemoryLimitExceeded { limit_percent: f64 },
    
    #[error("Invalid priority value: {priority}. Priority must be >= 0")]
    InvalidPriority { priority: i32 },
    
    #[error("Task execution failed: {message}")]
    TaskExecutionFailed { message: String },
    
    #[error("Resource limit reached: {resource} at {current}/{limit}")]
    ResourceLimitReached {
        resource: String,
        current: usize,
        limit: usize,
    },
    
    #[error("Invalid configuration: {message}")]
    InvalidConfiguration { message: String },
    
    #[error("Channel communication error: {message}")]
    ChannelError { message: String },
}

impl From<MakeParallelError> for PyErr {
    fn from(err: MakeParallelError) -> PyErr {
        PyException::new_err(err.to_string())
    }
}

/// Task error with detailed context
#[pyclass]
#[derive(Clone)]
pub struct TaskError {
    #[pyo3(get)]
    pub task_name: String,
    #[pyo3(get)]
    pub elapsed_time: f64,
    #[pyo3(get)]
    pub error_message: String,
    #[pyo3(get)]
    pub error_type: String,
    #[pyo3(get)]
    pub task_id: String,
}

#[pymethods]
impl TaskError {
    pub fn __str__(&self) -> String {
        format!(
            "TaskError in '{}' (task_id: {}, elapsed: {}s): {} ({})",
            self.task_name, self.task_id, self.elapsed_time, 
            self.error_message, self.error_type
        )
    }

    pub fn __repr__(&self) -> String {
        self.__str__()
    }
}
