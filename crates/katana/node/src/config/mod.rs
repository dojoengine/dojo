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

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub chain: ChainSpec,
    pub db: DbConfig,
    pub rpc: RpcConfig,
    pub metrics: Option<MetricsConfig>,
    pub starknet: StarknetConfig,
    pub messaging: Option<MessagingConfig>,
    pub sequencing: SequencingConfig,
    pub dev: DevConfig,
}

#[derive(Debug, Clone, Default)]
pub struct SequencingConfig {
    pub block_time: Option<u64>,
    pub no_mining: bool,
}
