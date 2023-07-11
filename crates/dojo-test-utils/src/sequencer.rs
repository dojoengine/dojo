use std::sync::Arc;

use jsonrpsee::core::Error;
use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::sequencer::KatanaSequencer;
pub use katana_core::sequencer::SequencerConfig;
use katana_rpc::config::ServerConfig;
use katana_rpc::{spawn, KatanaApi, NodeHandle, StarknetApi};
use starknet::accounts::SingleOwnerAccount;
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
    pub sequencer: Arc<KatanaSequencer>,
}

impl TestSequencer {
    pub async fn start(config: SequencerConfig) -> Self {
        let sequencer = Arc::new(KatanaSequencer::new(
            config,
            StarknetConfig {
                allow_zero_max_fee: true,
                env: Environment { chain_id: "SN_GOERLI".into(), ..Default::default() },
                ..Default::default()
            },
        ));

        sequencer.start().await;

        let starknet_api = StarknetApi::new(sequencer.clone());
        let katana_api = KatanaApi::new(sequencer.clone());

        let handle =
            spawn(katana_api, starknet_api, ServerConfig { port: 0, host: "localhost".into() })
                .await
                .expect("Unable to spawn server");

        let url = Url::parse(&format!("http://{}", handle.addr)).expect("Failed to parse URL");

        let account = sequencer.starknet.read().await.predeployed_accounts.accounts[0].clone();
        let account = TestAccount {
            private_key: FieldElement::from(account.private_key),
            account_address: FieldElement::from(*account.account_address.0.key()),
        };

        TestSequencer { sequencer, account, handle, url }
    }

    pub fn account(&self) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        SingleOwnerAccount::new(
            JsonRpcClient::new(HttpTransport::new(self.url.clone())),
            LocalWallet::from_signing_key(SigningKey::from_secret_scalar(self.account.private_key)),
            self.account.account_address,
            chain_id::TESTNET,
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
