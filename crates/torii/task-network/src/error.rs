use thiserror::Error;
use tokio::sync::AcquireError;
use tokio::task::JoinError;
use torii_adigraphmap::error::AcyclicDigraphMapError;

/// Errors that can occur in the task-network crate
#[derive(Error, Debug)]
pub enum TaskNetworkError {
    /// Error from the dependency graph
    #[error("Graph error: {0}")]
    GraphError(AcyclicDigraphMapError),
    /// Error from the semaphore
    #[error("Semaphore error: {0}")]
    SemaphoreError(AcquireError),
    /// Error from the join all
    #[error("Join error: {0}")]
    JoinError(JoinError),
}