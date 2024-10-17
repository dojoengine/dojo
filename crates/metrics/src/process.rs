use crate::prometheus_exporter::LOG_TARGET;
use metrics::{describe_gauge, gauge};

#[cfg(all(feature = "jemalloc", unix))]
pub fn collect_memory_stats() {
    use jemalloc_ctl::{epoch, stats};

    if epoch::advance()
        .map_err(|error| {
            tracing::error!(
                target: LOG_TARGET,
                error = %error,
                "Advance jemalloc epoch."
            )
        })
        .is_err()
    {
        return;
    }

    if let Ok(value) = stats::active::read().map_err(|error| {
        tracing::error!(
            target: LOG_TARGET,
            error = %error,
            "Read jemalloc.stats.active."
        )
    }) {
        gauge!("jemalloc.active").increment(value as f64);
    }

    if let Ok(value) = stats::allocated::read().map_err(|error| {
        tracing::error!(
            target: LOG_TARGET,
            error = %error,
            "Read jemalloc.stats.allocated."
        )
    }) {
        gauge!("jemalloc.allocated").increment(value as f64);
    }

    if let Ok(value) = stats::mapped::read().map_err(|error| {
        tracing::error!(
            target: LOG_TARGET,
            error = %error,
            "Read jemalloc.stats.mapped."
        )
    }) {
        gauge!("jemalloc.mapped").increment(value as f64);
    }

    if let Ok(value) = stats::metadata::read().map_err(|error| {
        tracing::error!(
            target: LOG_TARGET,
            error = %error,
            "Read jemalloc.stats.metadata."
        )
    }) {
        gauge!("jemalloc.metadata").increment(value as f64);
    }

    if let Ok(value) = stats::resident::read().map_err(|error| {
        tracing::error!(
            target: LOG_TARGET,
            error = %error,
            "Read jemalloc.stats.resident."
        )
    }) {
        gauge!("jemalloc.resident").increment(value as f64);
    }

    if let Ok(value) = stats::retained::read().map_err(|error| {
        tracing::error!(
            target: LOG_TARGET,
            error = %error,
            "Read jemalloc.stats.retained."
        )
    }) {
        gauge!("jemalloc.retained").increment(value as f64);
    }
}

#[cfg(all(feature = "jemalloc", unix))]
pub fn describe_memory_stats() {
    describe_gauge!(
        "jemalloc.active",
        metrics::Unit::Bytes,
        "Total number of bytes in active pages allocated by the application"
    );
    describe_gauge!(
        "jemalloc.allocated",
        metrics::Unit::Bytes,
        "Total number of bytes allocated by the application"
    );
    describe_gauge!(
        "jemalloc.mapped",
        metrics::Unit::Bytes,
        "Total number of bytes in active extents mapped by the allocator"
    );
    describe_gauge!(
        "jemalloc.metadata",
        metrics::Unit::Bytes,
        "Total number of bytes dedicated to jemalloc metadata"
    );
    describe_gauge!(
        "jemalloc.resident",
        metrics::Unit::Bytes,
        "Total number of bytes in physically resident data pages mapped by the allocator"
    );
    describe_gauge!(
        "jemalloc.retained",
        metrics::Unit::Bytes,
        "Total number of bytes in virtual memory mappings that were retained rather than being \
         returned to the operating system via e.g. munmap(2)"
    );
}

#[cfg(not(all(feature = "jemalloc", unix)))]
pub fn collect_memory_stats() {}

#[cfg(not(all(feature = "jemalloc", unix)))]
pub fn describe_memory_stats() {}
