use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::sync::Arc;

use futures::future::BoxFuture;
use futures::FutureExt;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
pub use tokio_util::sync::WaitForCancellationFuture as WaitForShutdownFuture;
use tokio_util::task::TaskTracker;
use tracing::trace;

use crate::task::{TaskBuilder, TaskResult};

pub type TaskHandle<T> = JoinHandle<TaskResult<T>>;

/// Usage for this task manager is mainly to spawn tasks that can be cancelled, and captures
/// panicked tasks (which in the context of the task manager - a critical task) for graceful
/// shutdown.
///
/// # Spawning tasks
///
/// To spawn tasks on the manager, call [`TaskManager::task_spawner`] to get a [`TaskSpawner`]
/// instance. The [`TaskSpawner`] can then be used to spawn tasks on the manager.
///
/// # Tasks cancellation
///
/// When the manager is dropped, all tasks that have yet to complete will be cancelled.
#[derive(Debug)]
pub struct TaskManager {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    /// A handle to the Tokio runtime.
    handle: Handle,
    /// Keep track of currently running tasks.
    tracker: TaskTracker,
    /// Used to cancel all running tasks.
    ///
    /// This is passed to all the tasks spawned by the manager.
    on_cancel: CancellationToken,
}

impl TaskManager {
    /// Create a new [`TaskManager`] from the given Tokio runtime handle.
    pub fn new(handle: Handle) -> Self {
        Self {
            inner: Arc::new(Inner {
                handle,
                tracker: TaskTracker::new(),
                on_cancel: CancellationToken::new(),
            }),
        }
    }

    /// Create a new [`TaskManager`] from the ambient Tokio runtime.
    pub fn current() -> Self {
        Self::new(Handle::current())
    }

    /// Returns a [`TaskSpawner`] that can be used to spawn tasks on the manager.
    pub fn task_spawner(&self) -> TaskSpawner {
        TaskSpawner { inner: Arc::clone(&self.inner) }
    }

    /// Returns a future that can be awaited for the shutdown signal to be received.
    pub fn wait_for_shutdown(&self) -> WaitForShutdownFuture<'_> {
        self.inner.on_cancel.cancelled()
    }

    /// Shuts down the manager and wait until all currently running tasks are finished, either due
    /// to completion or cancellation.
    ///
    /// No task can be spawned on the manager after this method is called.
    pub fn shutdown(&self) -> ShutdownFuture<'_> {
        let fut = Box::pin(async {
            if !self.inner.on_cancel.is_cancelled() {
                self.inner.on_cancel.cancel();
            }

            self.wait_for_shutdown().await;

            // need to close the tracker first before waiting
            let _ = self.inner.tracker.close();
            self.inner.tracker.wait().await;
        });

        ShutdownFuture { fut }
    }

    /// Return the handle to the Tokio runtime that the manager is associated with.
    pub fn handle(&self) -> &Handle {
        &self.inner.handle
    }

    /// Wait until all spawned tasks are completed.
    #[cfg(test)]
    async fn wait(&self) {
        // need to close the tracker first before waiting
        let _ = self.inner.tracker.close();
        self.inner.tracker.wait().await;
        // reopen the tracker for spawning future tasks
        let _ = self.inner.tracker.reopen();
    }
}

/// A spawner for spawning tasks on the [`TaskManager`] that it was derived from.
///
/// This is the main way to spawn tasks on a [`TaskManager`]. It can only be created
/// by calling [`TaskManager::task_spawner`].
#[derive(Debug, Clone)]
pub struct TaskSpawner {
    /// A handle to the [`TaskManager`] that this spawner is associated with.
    inner: Arc<Inner>,
}

impl TaskSpawner {
    /// Returns a new [`TaskBuilder`] for building a task.
    pub fn build_task(&self) -> TaskBuilder<'_> {
        TaskBuilder::new(self)
    }

    pub(crate) fn spawn<F>(&self, fut: F) -> TaskHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.spawn_inner(fut)
    }

    pub(crate) fn cancellation_token(&self) -> &CancellationToken {
        &self.inner.on_cancel
    }

    fn spawn_inner<F>(&self, task: F) -> TaskHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let task = self.make_cancellable(task);
        let task = self.inner.tracker.track_future(task);
        self.inner.handle.spawn(task)
    }

    fn make_cancellable<F>(&self, fut: F) -> impl Future<Output = TaskResult<F::Output>>
    where
        F: Future,
    {
        let ct = self.inner.on_cancel.clone();
        async move {
            tokio::select! {
                _ = ct.cancelled() => {
                    TaskResult::Cancelled
                },
                res = fut => {
                    TaskResult::Completed(res)
                },
            }
        }
    }
}

impl Drop for TaskManager {
    fn drop(&mut self) {
        trace!(target: "tasks", "Task manager is dropped, cancelling all ongoing tasks.");
        self.inner.on_cancel.cancel();
    }
}

/// A futures that resolves when the [TaskManager] is shutdown.
#[must_use = "futures do nothing unless polled"]
pub struct ShutdownFuture<'a> {
    fut: BoxFuture<'a, ()>,
}

impl<'a> Future for ShutdownFuture<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().fut.poll_unpin(cx)
    }
}

impl<'a> core::fmt::Debug for ShutdownFuture<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShutdownFuture").field("fut", &"...").finish()
    }
}

#[cfg(test)]
mod tests {
    use futures::future;
    use tokio::time::{self, Duration};

    use super::*;

    #[tokio::test]
    async fn normal_tasks() {
        let manager = TaskManager::current();
        let spawner = manager.task_spawner();

        spawner.build_task().spawn(time::sleep(Duration::from_secs(1)));
        spawner.build_task().spawn(time::sleep(Duration::from_secs(1)));
        spawner.build_task().spawn(time::sleep(Duration::from_secs(1)));

        // 3 tasks should be spawned on the manager
        assert_eq!(manager.inner.tracker.len(), 3);

        // wait until all task spawned to the manager have been completed
        manager.wait().await;

        assert!(
            !manager.inner.on_cancel.is_cancelled(),
            "cancellation signal shouldn't be sent on normal task completion"
        )
    }

    #[tokio::test]
    async fn task_with_graceful_shutdown() {
        let manager = TaskManager::current();
        let spawner = manager.task_spawner();

        // mock long running normal task and a task with graceful shutdown
        spawner.build_task().spawn(async {
            loop {
                time::sleep(Duration::from_secs(1)).await
            }
        });

        spawner.build_task().spawn(async {
            loop {
                time::sleep(Duration::from_secs(1)).await
            }
        });

        // assert that 2 tasks should've been spawned
        assert_eq!(manager.inner.tracker.len(), 2);

        // Spawn a task with graceful shuwdown that finish immediately.
        // The long running task should be cancelled due to the graceful shutdown.
        spawner.build_task().graceful_shutdown().spawn(future::ready(()));

        // wait until all task spawned to the manager have been completed
        manager.shutdown().await;
    }

    #[tokio::test]
    async fn critical_task_implicit_graceful_shutdown() {
        let manager = TaskManager::current();
        manager.task_spawner().build_task().critical().spawn(future::ready(()));
        manager.shutdown().await;
    }

    #[tokio::test]
    async fn critical_task_graceful_shudown_on_panicked() {
        let manager = TaskManager::current();
        manager.task_spawner().build_task().critical().spawn(async { panic!("panicking") });
        manager.shutdown().await;
    }
}
