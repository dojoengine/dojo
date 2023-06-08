use std::sync::Arc;

use jsonrpsee::core::Error;
use jsonrpsee::server::ServerHandle;
use katana_core::sequencer::KatanaSequencer;
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

const ACCOUNT_ADDRESS: FieldElement = FieldElement::from_mont([
    17796827018638541424,
    17823930040347334339,
    3831215415084348690,
    569269283564471682,
]);

const ACCOUNT_PK: FieldElement = FieldElement::from_mont([
    1113403281716850492,
    5176054014765279300,
    6075185975736318605,
    165462628152687232,
]);

pub struct TestSequencer {
    handle: ServerHandle,
    sequencer: Arc<KatanaSequencer>,
    account: SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
}

impl TestSequencer {
    pub async fn start() -> Self {
        let sequencer = Arc::new(KatanaSequencer::new(StarknetConfig {
            total_accounts: 1,
            allow_zero_max_fee: true,
            chain_id: "SN_GOERLI".into(),
            ..Default::default()
        }));

        sequencer.start().await;

        let server = KatanaNodeRpc::new(sequencer.clone(), RpcConfig { port: 0 });
        let (socket_addr, handle) = server.run().await.unwrap();

        let url = Url::parse(&format!("http://{}", socket_addr)).expect("Failed to parse URL");

        let account = sequencer.starknet.read().await.predeployed_accounts.accounts[0].clone();
        let account = SingleOwnerAccount::new(
            JsonRpcClient::new(HttpTransport::new(url)),
            LocalWallet::from_signing_key(SigningKey::from_secret_scalar(FieldElement::from(
                account.private_key,
            ))),
            FieldElement::from(*account.account_address.0.key()),
            chain_id::TESTNET,
        );

        TestSequencer { sequencer, account, handle }
    }

    pub fn account(&self) -> &SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        &self.account
    }

    pub fn stop(self) -> Result<(), Error> {
        self.handle.stop()
    }
}
