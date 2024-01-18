use std::future::Future;
use std::pin::Pin;
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

#[derive(Debug)]
#[must_use = "BlockingTaskHandle does nothing unless polled"]
pub struct BlockingTaskHandle<T>(oneshot::Receiver<T>);

impl<T> Future for BlockingTaskHandle<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.get_mut().0).poll(cx) {
            Poll::Ready(Ok(res)) => Poll::Ready(res),
            Poll::Ready(Err(_)) => panic!("blocking task cancelled"),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// For expensive CPU-bound computations. For spawing blocking IO-bound tasks, use
/// [TokioTaskSpawner::spawn_blocking].
pub struct BlockingTaskPool {
    pool: rayon::ThreadPool,
}

impl BlockingTaskPool {
    pub fn new() -> Self {
        Self { pool: Self::build().build().unwrap() }
    }

    pub fn build() -> ThreadPoolBuilder {
        ThreadPoolBuilder::new().thread_name(|i| format!("blocking-thread-pool-{i}"))
    }

    pub fn spawn<F, T>(&self, func: F) -> BlockingTaskHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = oneshot::channel::<T>();
        self.pool.spawn(move || {
            let res = func();
            let _ = tx.send(res);
        });
        BlockingTaskHandle(rx)
    }
}
