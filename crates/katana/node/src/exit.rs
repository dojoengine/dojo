use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Result;
use futures::future::BoxFuture;
use futures::FutureExt;

use crate::LaunchedNode;

/// A Future that is resolved once the node has been stopped including all of its running tasks.
#[must_use = "futures do nothing unless polled"]
pub struct NodeStoppedFuture<'a> {
    fut: BoxFuture<'a, Result<()>>,
}

impl<'a> NodeStoppedFuture<'a> {
    pub(crate) fn new(handle: &'a LaunchedNode) -> Self {
        let fut = Box::pin(async {
            handle.node.task_manager.wait_for_shutdown().await;
            handle.rpc.handle.clone().stopped().await;
            Ok(())
        });
        Self { fut }
    }
}

impl<'a> Future for NodeStoppedFuture<'a> {
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.fut.poll_unpin(cx)
    }
}

impl<'a> core::fmt::Debug for NodeStoppedFuture<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NodeStoppedFuture").field("fut", &"...").finish()
    }
}
