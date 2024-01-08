//! Transaction pool metrics.

use dojo_metrics::{
    metrics::Counter,
    Metrics,
};

#[derive(Metrics)]
#[metrics(scope = "core")]
pub struct TxPoolMetrics {
    /// Number of transactions inserted in the pool
    #[metric(describe = "Number of transactions inserted in the pool.")]
    pub(crate) inserted_transactions: Counter,
}