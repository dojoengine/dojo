use std::sync::Arc;

use jsonrpsee::core::Error;
pub use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::backend::Backend;
#[allow(deprecated)]
pub use katana_core::sequencer::SequencerConfig;
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_node::NodeHandle;
use katana_primitives::chain::ChainId;
use katana_rpc::config::ServerConfig;
use katana_rpc_api::ApiKind;
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
    handle: NodeHandle,
    account: TestAccount,
    backend: Arc<Backend<BlockifierFactory>>,
}

impl TestSequencer {
    #[allow(deprecated)]
    pub async fn start(config: SequencerConfig, starknet_config: StarknetConfig) -> Self {
        let server_config = ServerConfig {
            port: 0,
            host: "127.0.0.1".into(),
            max_connections: 100,
            allowed_origins: None,
            apis: vec![
                ApiKind::Starknet,
                ApiKind::Katana,
                ApiKind::Dev,
                ApiKind::Saya,
                ApiKind::Torii,
            ],
        };

        let (handle, backend) = katana_node::start(server_config, config, starknet_config)
            .await
            .expect("Failed to build node components");

        let url = Url::parse(&format!("http://{}", handle.addr)).expect("Failed to parse URL");

        let account = backend.config.genesis.accounts().next().unwrap();
        let account = TestAccount {
            private_key: Felt::from_bytes_be(&account.1.private_key().unwrap().to_bytes_be()),
            account_address: Felt::from_bytes_be(&account.0.to_bytes_be()),
        };

        TestSequencer { backend, account, handle, url }
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
        &self.backend
    }

    pub fn account_at_index(
        &self,
        index: usize,
    ) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        #[allow(deprecated)]
        let accounts: Vec<_> = self.backend.config.genesis.accounts().collect::<_>();

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
        self.handle.handle.stop()
    }

    pub fn url(&self) -> Url {
        self.url.clone()
    }
}

pub fn get_default_test_starknet_config() -> StarknetConfig {
    StarknetConfig {
        disable_fee: true,
        env: Environment { chain_id: ChainId::SEPOLIA, ..Default::default() },
        ..Default::default()
    }
}
