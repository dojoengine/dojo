use std::sync::Arc;

use anyhow::{Context, Result};
use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::constants::{
    DEFAULT_GAS_PRICE, DEFAULT_INVOKE_MAX_STEPS, DEFAULT_VALIDATE_MAX_STEPS,
};
use katana_core::sequencer::{KatanaSequencer, SequencerConfig};
use katana_core::{self};
use katana_rpc::api::ApiKind;
use katana_rpc::config::ServerConfig;
use katana_rpc::{self, spawn, NodeHandle};
use lazy_static::lazy_static;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use url::Url;

use crate::builder::KatanaRunnerConfig;

lazy_static! {
    static ref SERVER_CONFIG: ServerConfig = ServerConfig {
        port: 0,
        host: "127.0.0.1".to_string(),
        max_connections: 1000,
        apis: vec![ApiKind::Starknet, ApiKind::Katana],
    };
    static ref STARKNET_ENVIRONMENT: Environment = Environment {
        chain_id: "KATANA".to_string(),
        gas_price: DEFAULT_GAS_PRICE,
        invoke_max_steps: DEFAULT_INVOKE_MAX_STEPS,
        validate_max_steps: DEFAULT_VALIDATE_MAX_STEPS,
    };
    static ref STARKNET_CONFIG: StarknetConfig = StarknetConfig {
        total_accounts: 10,
        seed: [0; 32],
        disable_fee: true,
        init_state: None,
        fork_rpc_url: None,
        fork_block_number: None,
        env: STARKNET_ENVIRONMENT.clone(),
    };
}

pub struct KatanaCompiled {
    pub sequencer: Arc<KatanaSequencer>,
    pub server_handle: NodeHandle,
}

impl KatanaCompiled {
    pub fn address(&self) -> String {
        format!("http://{}", self.server_handle.addr)
    }
}

impl KatanaCompiled {
    /// Use `KatanaRunnerBuilder` to create an instance
    pub async fn run(config: KatanaRunnerConfig) -> Result<(Self, JsonRpcClient<HttpTransport>)> {
        let starknet_config = STARKNET_CONFIG.clone();
        let mut server_config = SERVER_CONFIG.clone();

        server_config.port = config.port.unwrap_or(0);

        let sequencer_config = SequencerConfig::default();
        let sequencer = Arc::new(KatanaSequencer::new(sequencer_config, starknet_config).await);

        let server_handle = spawn(Arc::clone(&sequencer), server_config).await?;

        let url = Url::parse(&format!("http://{}/", server_handle.addr))
            .context("Failed to parse url")?;
        let provider = JsonRpcClient::new(HttpTransport::new(url));

        Ok((KatanaCompiled { sequencer, server_handle }, provider))
    }
}
