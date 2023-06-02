use std::sync::Arc;

use jsonrpsee::core::Error;
use jsonrpsee::server::ServerHandle;
use katana_core::sequencer::KatanaSequencer;
use katana_core::starknet::StarknetConfig;
use katana_rpc::config::RpcConfig;
use katana_rpc::KatanaNodeRpc;
use starknet::core::types::FieldElement;
use tokio::sync::RwLock;
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

pub struct Account {
    pub private_key: FieldElement,
    pub address: FieldElement,
}

pub struct Sequencer {
    url: Url,
    handle: ServerHandle,
}

impl Sequencer {
    pub async fn start() -> Sequencer {
        let sequencer = Arc::new(RwLock::new(KatanaSequencer::new(StarknetConfig {
            total_accounts: 1,
            allow_zero_max_fee: true,
            ..StarknetConfig::default()
        })));
        sequencer.write().await.start();
        let (socket_addr, handle) =
            KatanaNodeRpc::new(sequencer.clone(), RpcConfig { port: 0 }).run().await.unwrap();
        let url = Url::parse(&format!("http://{}", socket_addr)).expect("Failed to parse URL");

        Sequencer { url, handle }
    }

    pub fn url(&self) -> Url {
        self.url.clone()
    }

    pub fn account(&self) -> Account {
        Account { address: ACCOUNT_ADDRESS, private_key: ACCOUNT_PK }
    }

    pub fn stop(&self) -> Result<(), Error> {
        self.handle.stop()
    }
}
