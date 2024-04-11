use dojo_metrics::Metrics;
use metrics::Counter;

pub(crate) struct ServiceMetrics {
    pub(crate) block_producer: BlockProducerMetrics,
}

#[derive(Metrics)]
#[metrics(scope = "block_producer")]
pub(crate) struct BlockProducerMetrics {
    /// The amount of L1 gas processed in a block.
    pub(crate) total_l1_gas_processed: Counter,
    /// The amount of Cairo steps processed in a block.
    pub(crate) total_cairo_steps_processed: Counter,
}
