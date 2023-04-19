use blockifier::state::state_api::StateReader;
use jsonrpsee::{
    core::{async_trait, Error},
    server::{ServerBuilder, ServerHandle},
};
use katana_core::sequencer::Sequencer;
use starknet_api::core::{ContractAddress, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use std::net::SocketAddr;
use util::to_trimmed_hex_string;

use crate::api::{KatanaApiError, KatanaApiServer};
pub mod api;
mod util;

pub struct KatanaRpc {
    sequencer: Sequencer,
}

impl KatanaRpc {
    pub fn new(sequencer: Sequencer) -> Self {
        Self { sequencer }
    }

    pub async fn run(self) -> Result<(SocketAddr, ServerHandle), Error> {
        let server = ServerBuilder::new()
            .build("127.0.0.1:0")
            .await
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))?;

        let addr = server.local_addr()?;
        let handle = server.start(self.into_rpc())?;

        Ok((addr, handle))
    }
}

#[async_trait]
impl KatanaApiServer for KatanaRpc {
    async fn chain_id(&self) -> Result<String, Error> {
        Ok(self.sequencer.block_context.chain_id.to_string())
    }

    async fn get_nonce(&self, contract_address: String) -> Result<String, Error> {
        let nonce = self
            .sequencer
            .state
            .lock()
            .unwrap()
            .get_nonce_at(ContractAddress(patricia_key!(contract_address.as_str())))
            .unwrap();

        Ok(to_trimmed_hex_string(nonce.0.bytes()))
    }

    async fn block_number(&self) -> Result<u64, Error> {
        Ok(self.sequencer.block_context.block_number.0)
    }
}

#[cfg(test)]
mod tests {
    use katana_core::sequencer::Sequencer;

    use crate::{api::KatanaApiServer, KatanaRpc};

    #[tokio::test]
    async fn chain_id_is_ok() {
        let rpc = KatanaRpc::new(Sequencer::new());
        let chain_id = rpc.chain_id().await.unwrap();
        assert_eq!(chain_id, "KATANA");
    }

    #[tokio::test]
    async fn nonce_is_ok() {
        let rpc = KatanaRpc::new(Sequencer::new());
        let nonce = rpc.get_nonce("0xdead".to_string()).await.unwrap();
        assert_eq!(nonce, "0x0");
    }

    #[tokio::test]
    async fn block_number_is_ok() {
        let rpc = KatanaRpc::new(Sequencer::new());
        let block_number = rpc.block_number().await.unwrap();
        assert_eq!(block_number, 0);
    }
}
