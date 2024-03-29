use std::any::Any;
use std::future::Future;
use std::panic::{self, AssertUnwindSafe};
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use futures::channel::oneshot;
use rayon::ThreadPoolBuilder;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

/// This `struct` is created by the [TokioTaskSpawner::new] method.
#[derive(Debug, thiserror::Error)]
#[error("Failed to initialize task spawner: {0}")]
pub struct TaskSpawnerInitError(tokio::runtime::TryCurrentError);

/// A task spawner for spawning tasks on a tokio runtime. This is simple wrapper around a tokio's
/// runtime [Handle] to easily spawn tasks on the runtime.
///
/// For running expensive CPU-bound tasks, use [BlockingTaskPool] instead.
#[derive(Debug, Clone)]
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

/// This `struct` is created by the [BlockingTaskPool::new] method.
#[derive(Debug, thiserror::Error)]
#[error("Failed to initialize blocking thread pool: {0}")]
pub struct BlockingTaskPoolInitError(rayon::ThreadPoolBuildError);

pub type BlockingTaskResult<T> = Result<T, Box<dyn Any + Send>>;

#[derive(Debug)]
#[must_use = "BlockingTaskHandle does nothing unless polled"]
pub struct BlockingTaskHandle<T>(oneshot::Receiver<BlockingTaskResult<T>>);

impl<T> Future for BlockingTaskHandle<T> {
    type Output = BlockingTaskResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.get_mut().0).poll(cx) {
            Poll::Ready(Ok(res)) => Poll::Ready(res),
            Poll::Ready(Err(cancelled)) => Poll::Ready(Err(Box::new(cancelled))),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// A thread-pool for spawning blocking tasks . This is a simple wrapper around *rayon*'s
/// thread-pool. This is mainly for executing expensive CPU-bound tasks. For spawing blocking
/// IO-bound tasks, use [TokioTaskSpawner::spawn_blocking] instead.
///
/// Refer to the [CPU-bound tasks and blocking code] section of the *tokio* docs and this [blog
/// post] for more information.
///
/// [CPU-bound tasks and blocking code]: https://docs.rs/tokio/latest/tokio/index.html#cpu-bound-tasks-and-blocking-code
/// [blog post]: https://ryhl.io/blog/async-what-is-blocking/
#[derive(Debug, Clone)]
pub struct BlockingTaskPool {
    pool: Arc<rayon::ThreadPool>,
}

impl BlockingTaskPool {
    /// Returns *rayon*'s [ThreadPoolBuilder] which can be used to build a new [BlockingTaskPool].
    pub fn build() -> ThreadPoolBuilder {
        ThreadPoolBuilder::new().thread_name(|i| format!("blocking-thread-pool-{i}"))
    }

    /// Creates a new [BlockingTaskPool] with the default configuration.
    pub fn new() -> Result<Self, BlockingTaskPoolInitError> {
        Self::build()
            .build()
            .map(|pool| Self { pool: Arc::new(pool) })
            .map_err(BlockingTaskPoolInitError)
    }

    /// Creates a new [BlockingTaskPool] with the given *rayon* thread pool.
    pub fn new_with_pool(rayon_pool: rayon::ThreadPool) -> Self {
        Self { pool: Arc::new(rayon_pool) }
    }

    /// Spawns an asynchronous task in this thread-pool, returning a handle for waiting on the
    /// result asynchronously.
    pub fn spawn<F, R>(&self, func: F) -> BlockingTaskHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        self.pool.spawn(move || {
            let _ = tx.send(panic::catch_unwind(AssertUnwindSafe(func)));
        });
        BlockingTaskHandle(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokio_task_spawner() {
        let rt = tokio::runtime::Runtime::new().unwrap();

        {
            rt.block_on(async {
                assert!(
                    TokioTaskSpawner::new().is_ok(),
                    "TokioTaskSpawner::new() should return Ok if within a tokio runtime"
                )
            });
        }

        {
            let tokio_handle = rt.handle().clone();
            rt.block_on(async move {
                let spawner = TokioTaskSpawner::new_with_handle(tokio_handle);
                let res = spawner.spawn(async { 1 + 1 }).await;
                assert!(res.is_ok());
            })
        }

        {
            assert!(
                TokioTaskSpawner::new()
                    .unwrap_err()
                    .to_string()
                    .contains("Failed to initialize task spawner:"),
                "TokioTaskSpawner::new() should return an error if not within a tokio runtime"
            );
        }
    }

    #[test]
    fn blocking_task_pool() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let blocking_pool = BlockingTaskPool::new().unwrap();
        rt.block_on(async {
            let res = blocking_pool.spawn(|| 1 + 1).await;
            assert!(res.is_ok());
            let res = blocking_pool.spawn(|| panic!("test")).await;
            assert!(res.is_err(), "panic'd task should be caught");
        })
    }
}
