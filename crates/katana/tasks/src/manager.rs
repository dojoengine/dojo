use std::future::Future;

use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::task::{TaskBuilder, TaskResult};

pub type TaskHandle<T> = JoinHandle<TaskResult<T>>;

/// Usage for this task manager is mainly to spawn tasks that can be cancelled, and captures
/// panicked tasks (which in the context of the task manager - a critical task) for graceful
/// shutdown.
#[derive(Debug, Clone)]
pub struct TaskManager {
    /// A handle to the Tokio runtime.
    handle: Handle,
    /// Keep track of currently running tasks.
    tracker: TaskTracker,
    /// Used to cancel all running tasks.
    ///
    /// This is passed to all the tasks spawned by the manager.
    pub(crate) on_cancel: CancellationToken,
}

impl TaskManager {
    /// Create a new [`TaskManager`] from the given Tokio runtime handle.
    pub fn new(handle: Handle) -> Self {
        Self { handle, tracker: TaskTracker::new(), on_cancel: CancellationToken::new() }
    }

    pub fn current() -> Self {
        Self::new(Handle::current())
    }

    pub fn spawn<F>(&self, fut: F) -> TaskHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.spawn_inner(fut)
    }

    /// Wait for the shutdown signal to be received.
    pub async fn wait_for_shutdown(&self) {
        self.on_cancel.cancelled().await;
    }

    /// Shuts down the manager and wait until all currently running tasks are finished, either due
    /// to completion or cancellation.
    ///
    /// No task can be spawned on the manager after this method is called.
    pub async fn shutdown(self) {
        if !self.on_cancel.is_cancelled() {
            self.on_cancel.cancel();
        }

        self.wait_for_shutdown().await;

        // need to close the tracker first before waiting
        let _ = self.tracker.close();
        self.tracker.wait().await;
    }

    /// Return the handle to the Tokio runtime that the manager is associated with.
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Returns a new [`TaskBuilder`] for building a task to be spawned on this manager.
    pub fn build_task(&self) -> TaskBuilder<'_> {
        TaskBuilder::new(self)
    }

    /// Wait until all spawned tasks are completed.
    #[cfg(test)]
    async fn wait(&self) {
        // need to close the tracker first before waiting
        let _ = self.tracker.close();
        self.tracker.wait().await;
        // reopen the tracker for spawning future tasks
        let _ = self.tracker.reopen();
    }

    fn spawn_inner<F>(&self, task: F) -> TaskHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let task = self.make_cancellable(task);
        let task = self.tracker.track_future(task);
        self.handle.spawn(task)
    }

    fn make_cancellable<F>(&self, fut: F) -> impl Future<Output = TaskResult<F::Output>>
    where
        F: Future,
    {
        let ct = self.on_cancel.clone();
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
        self.on_cancel.cancel();
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

        manager.spawn(time::sleep(Duration::from_secs(1)));
        manager.spawn(time::sleep(Duration::from_secs(1)));
        manager.spawn(time::sleep(Duration::from_secs(1)));

        // 3 tasks should be spawned on the manager
        assert_eq!(manager.tracker.len(), 3);

        // wait until all task spawned to the manager have been completed
        manager.wait().await;

        assert!(
            !manager.on_cancel.is_cancelled(),
            "cancellation signal shouldn't be sent on normal task completion"
        )
    }

    #[tokio::test]
    async fn task_with_graceful_shutdown() {
        let manager = TaskManager::current();

        // mock long running normal task and a task with graceful shutdown
        manager.build_task().spawn(async {
            loop {
                time::sleep(Duration::from_secs(1)).await
            }
        });

        manager.build_task().spawn(async {
            loop {
                time::sleep(Duration::from_secs(1)).await
            }
        });

        // assert that 2 tasks should've been spawned
        assert_eq!(manager.tracker.len(), 2);

        // Spawn a task with graceful shuwdown that finish immediately.
        // The long running task should be cancelled due to the graceful shutdown.
        manager.build_task().graceful_shutdown().spawn(future::ready(()));

        // wait until all task spawned to the manager have been completed
        manager.shutdown().await;
    }

    #[tokio::test]
    async fn critical_task_implicit_graceful_shutdown() {
        let manager = TaskManager::current();
        manager.build_task().critical().spawn(future::ready(()));
        manager.shutdown().await;
    }

    #[tokio::test]
    async fn critical_task_graceful_shudown_on_panicked() {
        let manager = TaskManager::current();
        manager.build_task().critical().spawn(async { panic!("panicking") });
        manager.shutdown().await;
    }
}
