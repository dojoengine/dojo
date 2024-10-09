use dojo_metrics::Metrics;
use metrics::Counter;

#[derive(Metrics)]
#[metrics(scope = "block_producer")]
pub(crate) struct BlockProducerMetrics {
    /// The amount of L1 gas processed in a block.
    pub(crate) l1_gas_processed_total: Counter,
    /// The amount of Cairo steps processed in a block.
    pub(crate) cairo_steps_processed_total: Counter,
}
