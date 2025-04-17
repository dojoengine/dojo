use thiserror::Error;

/// Errors that can occur when working with a DigraphMap
#[derive(Error, Debug)]
pub enum DigraphMapError {
    #[error("Node with key {0:?} not found")]
    NodeNotFound(String),
    #[error("Adding edge would create a cycle")]
    CycleDetected,
    #[error("Duplicate node key: {0:?}")]
    DuplicateKey(String),
}