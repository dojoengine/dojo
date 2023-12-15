pub mod prometheus_exporter;
pub mod utils;

#[cfg(all(feature = "jemalloc", unix))]
use jemallocator as _;

// We use jemalloc for performance reasons
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;
