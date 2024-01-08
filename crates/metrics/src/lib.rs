pub mod prometheus_exporter;
pub mod utils;
pub use metrics;
pub use metrics_util;
pub use dojo_metrics_derive::Metrics;

#[cfg(all(feature = "jemalloc", unix))]
use jemallocator as _;

// We use jemalloc for performance reasons
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;