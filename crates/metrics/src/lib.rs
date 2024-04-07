pub mod prometheus_exporter;

#[cfg(all(feature = "jemalloc", unix))]
use jemallocator as _;
/// Re-export the metrics crate
pub use metrics;
/// Re-export the metrics-process crate
pub use metrics_process;
/// Re-export the metrics derive macro
pub use reth_metrics_derive::Metrics;

// We use jemalloc for performance reasons
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;
