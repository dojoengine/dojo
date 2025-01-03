use std::sync::Arc;

pub mod chain;
pub mod db;
pub mod dev;
pub mod execution;
pub mod fork;
pub mod metrics;
pub mod rpc;

use db::DbConfig;
use dev::DevConfig;
use execution::ExecutionConfig;
use fork::ForkingConfig;
use katana_core::service::messaging::MessagingConfig;
use katana_primitives::chain_spec::ChainSpec;
use metrics::MetricsConfig;
use rpc::RpcConfig;

/// Node configurations.
///
/// List of all possible options that can be used to configure a node.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// The chain specification.
    pub chain: Arc<ChainSpec>,

    /// Database options.
    pub db: DbConfig,

    /// Forking options.
    pub forking: Option<ForkingConfig>,

    /// Rpc options.
    pub rpc: RpcConfig,

    /// Metrics options.
    pub metrics: Option<MetricsConfig>,

    /// Execution options.
    pub execution: ExecutionConfig,

    /// Messaging options.
    pub messaging: Option<MessagingConfig>,

    /// Sequencing options.
    pub sequencing: SequencingConfig,

    /// Development options.
    pub dev: DevConfig,
    // /// Provider url for gas price oracle
    // pub l1_provider_url: Option<Url>,
}

/// Configurations related to block production.
#[derive(Debug, Clone, Default)]
pub struct SequencingConfig {
    /// The time in milliseconds for a block to be produced.
    pub block_time: Option<u64>,

    /// Disable automatic block production.
    ///
    /// Allowing block to only be produced manually.
    pub no_mining: bool,
}
