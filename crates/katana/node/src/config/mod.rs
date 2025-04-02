use std::sync::Arc;

pub mod db;
pub mod dev;
pub mod execution;
pub mod fork;
pub mod metrics;
pub mod rpc;
pub mod sequencing;

use db::DbConfig;
use dev::DevConfig;
use execution::ExecutionConfig;
use fork::ForkingConfig;
use katana_chain_spec::ChainSpec;
use katana_messaging::MessagingConfig;
use metrics::MetricsConfig;
use rpc::RpcConfig;
use sequencing::SequencingConfig;
#[cfg(feature = "cartridge")]
use url::Url;

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

    /// Cartridge paymaster options.
    #[cfg(feature = "cartridge")]
    pub paymaster: Option<Paymaster>,
}

#[cfg(feature = "cartridge")]
#[derive(Debug, Clone)]
pub struct Paymaster {
    /// The root URL for the Cartridge API.
    pub cartridge_api_url: Url,
}
