use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;

use futures::future::Either;
use futures::{FutureExt, TryFutureExt};
use thiserror::Error;
use tokio_metrics::TaskMonitor;
use tracing::{debug, error};

use crate::manager::TaskHandle;
use crate::TaskSpawner;

/// A task result that can be either completed or cancelled.
#[derive(Debug, Copy, Clone)]
pub enum TaskResult<T> {
    /// The task completed successfully with the given result.
    Completed(T),
    /// The task was cancelled.
    Cancelled,
}

impl<T> TaskResult<T> {
    /// Returns true if the task was cancelled.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, TaskResult::Cancelled)
    }
}

/// A builder for building tasks to be spawned on the associated task manager.
///
/// Can only be created using [`TaskManager::build_task`].
#[derive(Debug)]
pub struct TaskBuilder<'a> {
    /// The task manager that the task will be spawned on.
    spawner: &'a TaskSpawner,
    /// The name of the task.
    name: Option<String>,
    /// Indicates whether the task should be instrumented.
    instrument: bool,
    /// Notifies the task manager to perform a graceful shutdown when the task is finished due to
    /// ompletion or cancellation.
    graceful_shutdown: bool,
}

impl<'a> TaskBuilder<'a> {
    /// Creates a new task builder associated with the given task manager.
    pub(crate) fn new(spawner: &'a TaskSpawner) -> Self {
        Self { spawner, name: None, instrument: false, graceful_shutdown: false }
    }

    pub fn critical(self) -> CriticalTaskBuilder<'a> {
        CriticalTaskBuilder { builder: self.graceful_shutdown() }
    }

    /// Sets the name of the task.
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Instruments the task for collecting metrics. Is a no-op for now.
    pub fn instrument(mut self) -> Self {
        self.instrument = true;
        self
    }

    /// Notifies the task manager to perform a graceful shutdown when the task is finished.
    pub fn graceful_shutdown(mut self) -> Self {
        self.graceful_shutdown = true;
        self
    }

    /// Spawns the given future based on the configured builder.
    pub fn spawn<F>(self, fut: F) -> TaskHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let Self { spawner, instrument, graceful_shutdown, name } = self;

        // creates a future that will send a cancellation signal to the manager when the future is
        // completed, regardless of success or error.
        let fut = {
            let ct = spawner.cancellation_token().clone();
            fut.map(move |res| {
                if graceful_shutdown {
                    debug!(target: "tasks", task = name, "Task with graceful shutdown completed.");
                    ct.cancel();
                }
                res
            })
        };

        let fut = if instrument {
            // TODO: store the TaskMonitor
            let monitor = TaskMonitor::new();
            Either::Left(monitor.instrument(fut))
        } else {
            Either::Right(fut)
        };

        spawner.spawn(fut)
    }
}

/// Builder for building critical tasks. This struct can only be created by calling
/// [`TaskBuilder::critical`]
#[derive(Debug)]
pub struct CriticalTaskBuilder<'a> {
    builder: TaskBuilder<'a>,
}

impl<'a> CriticalTaskBuilder<'a> {
    pub fn name(mut self, name: &str) -> Self {
        self.builder.name = Some(name.to_string());
        self
    }

    /// Instruments the task for collecting metrics. Is a no-op for now.
    pub fn instrument(mut self) -> Self {
        self.builder.instrument = true;
        self
    }

    pub fn spawn<F>(self, fut: F) -> TaskHandle<()>
    where
        F: Future + Send + 'static,
    {
        let task_name = self.builder.name.clone().unwrap_or("".to_string());
        let ct = self.builder.spawner.cancellation_token().clone();

        let fut = AssertUnwindSafe(fut)
            .catch_unwind()
            .map_err(move |error| {
                let error = PanickedTaskError { error };
                error!(%error, task = task_name, "Critical task failed.");
                ct.cancel();
                error
            })
            .map(drop);

        self.builder.spawn(fut)
    }
}

/// A simple wrapper type so that we can implement [`std::error::Error`] for `Box<dyn Any + Send>`.
#[derive(Debug, Error)]
pub struct PanickedTaskError {
    /// The error that caused the panic. It is a boxed `dyn Any` due to the future returned by
    /// [`catch_unwind`](futures::future::FutureExt::catch_unwind).
    error: Box<dyn Any + Send>,
}

impl std::fmt::Display for PanickedTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.error.downcast_ref::<String>() {
            None => Ok(()),
            Some(msg) => write!(f, "{msg}"),
        }
    }
}
