use std::sync::Arc;

use jsonrpsee::core::Error;
pub use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::constants::MAX_RECURSION_DEPTH;
use katana_core::env::get_default_vm_resource_fee_cost;
use katana_core::sequencer::KatanaSequencer;
pub use katana_core::sequencer::SequencerConfig;
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_executor::SimulationFlag;
use katana_primitives::chain::ChainId;
use katana_primitives::env::{CfgEnv, FeeTokenAddressses};
use katana_rpc::config::ServerConfig;
use katana_rpc::{spawn, NodeHandle};
use katana_rpc_api::ApiKind;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::chain_id;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};
use url::Url;

pub struct TestAccount {
    pub private_key: FieldElement,
    pub account_address: FieldElement,
}

#[allow(unused)]
pub struct TestSequencer {
    url: Url,
    handle: NodeHandle,
    account: TestAccount,
    pub sequencer: Arc<KatanaSequencer<BlockifierFactory>>,
}

impl TestSequencer {
    pub async fn start(config: SequencerConfig, starknet_config: StarknetConfig) -> Self {
        let cfg_env = CfgEnv {
            chain_id: starknet_config.env.chain_id,
            vm_resource_fee_cost: get_default_vm_resource_fee_cost(),
            invoke_tx_max_n_steps: starknet_config.env.invoke_max_steps,
            validate_max_n_steps: starknet_config.env.validate_max_steps,
            max_recursion_depth: MAX_RECURSION_DEPTH,
            fee_token_addresses: FeeTokenAddressses {
                eth: starknet_config.genesis.fee_token.address,
                strk: Default::default(),
            },
        };

        let simulation_flags = SimulationFlag {
            skip_validate: starknet_config.disable_validate,
            skip_fee_transfer: starknet_config.disable_fee,
            ..Default::default()
        };

        let executor_factory = BlockifierFactory::new(cfg_env, simulation_flags);

        let sequencer = Arc::new(
            KatanaSequencer::new(executor_factory, config, starknet_config)
                .await
                .expect("Failed to create sequencer"),
        );

        let handle = spawn(
            Arc::clone(&sequencer),
            ServerConfig {
                port: 0,
                host: "127.0.0.1".into(),
                max_connections: 100,
                apis: vec![ApiKind::Starknet, ApiKind::Katana, ApiKind::Dev],
            },
        )
        .await
        .expect("Unable to spawn server");

        let url = Url::parse(&format!("http://{}", handle.addr)).expect("Failed to parse URL");

        let account = sequencer.backend.config.genesis.accounts().next().unwrap();
        let account = TestAccount {
            private_key: account.1.private_key().unwrap(),
            account_address: (*account.0).into(),
        };

        TestSequencer { sequencer, account, handle, url }
    }

    pub fn account(&self) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        SingleOwnerAccount::new(
            JsonRpcClient::new(HttpTransport::new(self.url.clone())),
            LocalWallet::from_signing_key(SigningKey::from_secret_scalar(self.account.private_key)),
            self.account.account_address,
            chain_id::TESTNET,
            ExecutionEncoding::New,
        )
    }

    pub fn raw_account(&self) -> &TestAccount {
        &self.account
    }

    pub fn stop(self) -> Result<(), Error> {
        self.handle.handle.stop()
    }

    pub fn url(&self) -> Url {
        self.url.clone()
    }
}

pub fn get_default_test_starknet_config() -> StarknetConfig {
    StarknetConfig {
        disable_fee: true,
        env: Environment { chain_id: ChainId::GOERLI, ..Default::default() },
        ..Default::default()
    }
}
