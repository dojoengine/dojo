pub mod exporters;
mod process;
mod server;

use std::net::SocketAddr;

#[cfg(all(feature = "jemalloc", unix))]
use jemallocator as _;
/// Re-export the metrics crate
pub use metrics;
/// Re-export the metrics derive macro
pub use metrics_derive::Metrics;
/// Re-export the metrics-process crate
pub use metrics_process;
pub use server::*;

// We use jemalloc for performance reasons
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("global metrics recorder already installed.")]
    GlobalRecorderAlreadyInstalled,

    #[error("could not bind to address: {addr}")]
    FailedToBindAddress { addr: SocketAddr },

    #[error(transparent)]
    Server(#[from] hyper::Error),
}

/// A helper trait for defining the type for hooks that are called when the metrics are being
/// collected by the server.
pub trait Hook: Fn() + Send + Sync {}
impl<T: Fn() + Send + Sync> Hook for T {}

/// A boxed [`Hook`].
pub type BoxedHook<T> = Box<dyn Hook<Output = T>>;
/// A list of [BoxedHook].
pub type Hooks = Vec<BoxedHook<()>>;

pub trait Exporter: Clone + Send + Sync {
    fn export(&self) -> String;
}

/// A helper trait for reporting metrics.
///
/// This is meant for types that require a specific trigger to register their metrics.
pub trait Report: Send + Sync {
    /// Report the metrics.
    fn report(&self);
}
