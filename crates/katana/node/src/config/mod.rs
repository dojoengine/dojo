pub mod db;
pub mod dev;
pub mod metrics;
pub mod rpc;

use db::DbConfig;
use dev::DevConfig;
use katana_core::backend::config::StarknetConfig;
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
    pub chain: ChainSpec,

    /// Database options.
    pub db: DbConfig,

    /// Rpc options.
    pub rpc: RpcConfig,

    /// Metrics options.
    pub metrics: Option<MetricsConfig>,

    /// Starknet options.
    pub starknet: StarknetConfig,

    /// Messaging options.
    pub messaging: Option<MessagingConfig>,

    /// Sequencing options.
    pub sequencing: SequencingConfig,

    /// Development options.
    pub dev: DevConfig,
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
