pub mod prometheus;

/// Trait for metrics recorder whose metrics can be exported.
pub trait Exporter: Clone + Send + Sync {
    /// Export the metrics that have been recorded by the metrics thus far.
    fn export(&self) -> String;
}
