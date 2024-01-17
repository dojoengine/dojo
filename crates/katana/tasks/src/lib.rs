use std::future::Future;

use tokio::runtime::Handle;
use tokio::task::JoinHandle;

/// This `struct` is created by the [TokioTaskSpawner::new] method.
#[derive(Debug, thiserror::Error)]
#[error("Failed to initialize task spawner: {0}")]
pub struct TaskSpawnerInitError(tokio::runtime::TryCurrentError);

/// A task spawner for spawning tasks on a tokio runtime. This is simple wrapper around a tokio's
/// runtime [Handle] to easily spawn tasks on the runtime.
#[derive(Clone)]
pub struct TokioTaskSpawner {
    /// Handle to the tokio runtime.
    tokio_handle: Handle,
}

impl TokioTaskSpawner {
    /// Creates a new [TokioTaskSpawner] over the currently running tokio runtime.
    ///
    /// ## Errors
    ///
    /// Returns an error if no tokio runtime has been started.
    pub fn new() -> Result<Self, TaskSpawnerInitError> {
        Ok(Self { tokio_handle: Handle::try_current().map_err(TaskSpawnerInitError)? })
    }

    /// Creates a new [TokioTaskSpawner] with the given tokio runtime [Handle].
    pub fn new_with_handle(tokio_handle: Handle) -> Self {
        Self { tokio_handle }
    }
}

impl TokioTaskSpawner {
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.tokio_handle.spawn(future)
    }

    pub fn spawn_blocking<F, R>(&self, func: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.tokio_handle.spawn_blocking(func)
    }
}
