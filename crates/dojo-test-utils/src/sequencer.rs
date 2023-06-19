use std::sync::Arc;

use jsonrpsee::core::Error;
use jsonrpsee::server::ServerHandle;
use katana_core::sequencer::{KatanaSequencer, SequencerConfig};
use katana_core::starknet::StarknetConfig;
use katana_rpc::config::RpcConfig;
use katana_rpc::KatanaNodeRpc;
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
    handle: ServerHandle,
    account: TestAccount,
    pub sequencer: Arc<KatanaSequencer>,
}

impl TestSequencer {
    pub async fn start() -> Self {
        let sequencer = Arc::new(KatanaSequencer::new(
            SequencerConfig::default(),
            StarknetConfig {
                auto_mine: true,
                total_accounts: 1,
                allow_zero_max_fee: true,
                chain_id: "SN_GOERLI".into(),
                ..Default::default()
            },
        ));

        sequencer.start().await;

        let server = KatanaNodeRpc::new(sequencer.clone(), RpcConfig { port: 0 });
        let (socket_addr, handle) = server.run().await.unwrap();

        let url = Url::parse(&format!("http://{}", socket_addr)).expect("Failed to parse URL");

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
        self.handle.stop()
    }

    pub fn url(&self) -> Url {
        self.url.clone()
    }
}
