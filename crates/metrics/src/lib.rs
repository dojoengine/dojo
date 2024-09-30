pub mod prometheus_exporter;

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

/// A helper trait for reporting metrics.
///
/// This is meant for types that require a specific trigger to register their metrics.
pub trait Report: Send + Sync {
    /// Report the metrics.
    fn report(&self);
}
