use std::collections::HashSet;
use std::sync::Arc;

use jsonrpsee::core::Error;
use katana_core::backend::Backend;
use katana_core::constants::DEFAULT_SEQUENCER_ADDRESS;
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_node::config::dev::DevConfig;
use katana_node::config::rpc::{ApiKind, RpcConfig, DEFAULT_RPC_ADDR, DEFAULT_RPC_MAX_CONNECTIONS};
pub use katana_node::config::*;
use katana_node::LaunchedNode;
use katana_primitives::chain::ChainId;
use katana_primitives::chain_spec::ChainSpec;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::chain_id;
use starknet::core::types::{BlockId, BlockTag, Felt};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};
use url::Url;

#[derive(Debug)]
pub struct TestAccount {
    pub private_key: Felt,
    pub account_address: Felt,
}

#[allow(unused)]
#[allow(missing_debug_implementations)]
pub struct TestSequencer {
    url: Url,
    handle: LaunchedNode,
    account: TestAccount,
}

impl TestSequencer {
    pub async fn start(config: Config) -> Self {
        let handle = katana_node::build(config)
            .await
            .expect("Failed to build node components")
            .launch()
            .await
            .expect("Failed to launch node");

        let url = Url::parse(&format!("http://{}", handle.rpc.addr)).expect("Failed to parse URL");

        let account = handle.node.backend.chain_spec.genesis.accounts().next().unwrap();
        let account = TestAccount {
            private_key: Felt::from_bytes_be(&account.1.private_key().unwrap().to_bytes_be()),
            account_address: Felt::from_bytes_be(&account.0.to_bytes_be()),
        };

        TestSequencer { handle, account, url }
    }

    pub fn account(&self) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        let mut account = SingleOwnerAccount::new(
            JsonRpcClient::new(HttpTransport::new(self.url.clone())),
            LocalWallet::from_signing_key(SigningKey::from_secret_scalar(self.account.private_key)),
            self.account.account_address,
            chain_id::SEPOLIA,
            ExecutionEncoding::New,
        );

        account.set_block_id(starknet::core::types::BlockId::Tag(BlockTag::Pending));

        account
    }

    pub fn provider(&self) -> JsonRpcClient<HttpTransport> {
        JsonRpcClient::new(HttpTransport::new(self.url.clone()))
    }

    pub fn backend(&self) -> &Arc<Backend<BlockifierFactory>> {
        &self.handle.node.backend
    }

    pub fn account_at_index(
        &self,
        index: usize,
    ) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        let accounts: Vec<_> =
            self.handle.node.backend.chain_spec.genesis.accounts().collect::<_>();

        let account = accounts[index];
        let private_key = Felt::from_bytes_be(&account.1.private_key().unwrap().to_bytes_be());
        let address: Felt = Felt::from_bytes_be(&account.0.to_bytes_be());

        let mut account = SingleOwnerAccount::new(
            JsonRpcClient::new(HttpTransport::new(self.url.clone())),
            LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private_key)),
            address,
            chain_id::SEPOLIA,
            ExecutionEncoding::New,
        );

        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        account
    }

    pub fn raw_account(&self) -> &TestAccount {
        &self.account
    }

    pub fn stop(self) -> Result<(), Error> {
        self.handle.rpc.handle.stop()
    }

    pub fn url(&self) -> Url {
        self.url.clone()
    }
}

pub fn get_default_test_config(sequencing: SequencingConfig) -> Config {
    let dev = DevConfig { fee: false, account_validation: true, fixed_gas_prices: None };
    let mut chain = ChainSpec { id: ChainId::SEPOLIA, ..Default::default() };
    chain.genesis.sequencer_address = *DEFAULT_SEQUENCER_ADDRESS;

    let rpc = RpcConfig {
        cors_domain: None,
        port: 0,
        addr: DEFAULT_RPC_ADDR,
        max_connections: DEFAULT_RPC_MAX_CONNECTIONS,
        apis: HashSet::from([ApiKind::Starknet, ApiKind::Dev, ApiKind::Saya, ApiKind::Torii]),
    };

    Config { sequencing, rpc, dev, chain, ..Default::default() }
}
