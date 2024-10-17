mod process;
mod prometheus_exporter;

pub use prometheus_exporter::*;

#[cfg(all(feature = "jemalloc", unix))]
use jemallocator as _;
/// Re-export the metrics crate
pub use metrics;
/// Re-export the metrics derive macro
pub use metrics_derive::Metrics;
/// Re-export the metrics-process crate
pub use metrics_process;

// We use jemalloc for performance reasons
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

/// A helper trait for defining the type for hooks that are called when the metrics are being collected
/// by the server.
pub trait Hook: Fn() + Send + Sync {}
impl<T: Fn() + Send + Sync> Hook for T {}

/// A boxed [`Hook`].
pub type BoxedHook<T> = Box<dyn Hook<Output = T>>;
/// A list of [BoxedHook].
pub type Hooks = Vec<BoxedHook<()>>;

/// A helper trait for reporting metrics.
///
/// This is meant for types that require a specific trigger to register their metrics.
pub trait Report: Send + Sync {
    /// Report the metrics.
    fn report(&self);
}
