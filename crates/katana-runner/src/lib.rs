use anyhow::Result;
use std::sync::Arc;

use katana_core::{
    self,
    backend::config::{Environment, StarknetConfig},
    constants::{DEFAULT_GAS_PRICE, DEFAULT_INVOKE_MAX_STEPS, DEFAULT_VALIDATE_MAX_STEPS},
    sequencer::{KatanaSequencer, SequencerConfig},
};
use katana_rpc::{self, api::ApiKind, config::ServerConfig, spawn, NodeHandle};

use lazy_static::lazy_static;

use crate::binary::KatanaRunner;

mod binary;

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

pub struct KatanaGuard {
    pub sequencer: Arc<KatanaSequencer>,
    pub server_handle: NodeHandle,
}

impl KatanaGuard {
    pub fn address(&self) -> String {
        format!("http://{}", self.server_handle.addr)
    }
}

pub async fn run() -> Result<KatanaGuard> {
    let starknet_config = StarknetConfig {
        total_accounts: 10,
        seed: [0; 32],
        disable_fee: true,
        init_state: None,
        fork_rpc_url: None,
        fork_block_number: None,
        env: STARKNET_ENVIRONMENT.clone(),
    };

    let sequencer_config = SequencerConfig::default();
    let sequencer = Arc::new(KatanaSequencer::new(sequencer_config, starknet_config).await);

    let server_handle = spawn(Arc::clone(&sequencer), SERVER_CONFIG.clone()).await?;

    Ok(KatanaGuard { sequencer, server_handle })
}

#[tokio::test]
async fn test_run() {
    loop {
        let guard = run().await.unwrap();
        println!("Restarting server");
        println!("Dropping server on {}", guard.address());
    }
}

#[tokio::test]
async fn test_run_binary() {
    loop {
        let a = KatanaRunner::new();

        let guard = run().await.unwrap();
        println!("Restarting server");
        println!("Dropping server on {}", guard.address());
    }
}
