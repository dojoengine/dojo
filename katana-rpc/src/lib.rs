use blockifier::state::state_api::StateReader;
use jsonrpsee::{
    core::{async_trait, Error},
    server::{ServerBuilder, ServerHandle},
    types::error::CallError,
};
use katana_core::sequencer::KatanaSequencer;
use starknet::{core::types::FieldElement, providers::jsonrpc::models::DeployTransactionResult};
use starknet_api::{
    core::{ClassHash, ContractAddress, PatriciaKey},
    hash::StarkFelt,
    stark_felt,
    transaction::{Calldata, ContractAddressSalt, TransactionVersion},
};
use starknet_api::{hash::StarkHash, transaction::TransactionSignature};
use starknet_api::{patricia_key, state::StorageKey};
use std::{net::SocketAddr, sync::Arc};
use util::to_trimmed_hex_string;

use crate::api::{KatanaApiError, KatanaApiServer};
pub mod api;
mod util;

pub struct KatanaRpc {
    sequencer: Arc<KatanaSequencer>,
}

impl KatanaRpc {
    pub fn new(sequencer: KatanaSequencer) -> Self {
        Self {
            sequencer: Arc::new(sequencer),
        }
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

    async fn add_deploy_account_transaction(
        &self,
        contract_class: String,
        version: String,
        contract_address_salt: String,
        constructor_calldata: Vec<String>,
    ) -> Result<DeployTransactionResult, Error> {
        let (transaction_hash, contract_address) = self
            .sequencer
            .deploy_account(
                ClassHash(stark_felt!(contract_class.as_str())),
                TransactionVersion(stark_felt!(version.as_str())),
                ContractAddressSalt(stark_felt!(contract_address_salt.as_str())),
                Calldata(Arc::new(
                    constructor_calldata
                        .iter()
                        .map(|calldata| stark_felt!(calldata.as_str()))
                        .collect(),
                )),
                TransactionSignature::default(),
            )
            .map_err(|e| Error::Call(CallError::Failed(anyhow::anyhow!(e.to_string()))))?;

        Ok(DeployTransactionResult {
            transaction_hash: FieldElement::from_byte_slice_be(transaction_hash.0.bytes())
                .map_err(|_| Error::from(KatanaApiError::InternalServerError))?,
            contract_address: FieldElement::from_byte_slice_be(contract_address.0.key().bytes())
                .map_err(|_| Error::from(KatanaApiError::InternalServerError))?,
        })
    }

    async fn get_storage_at(
        &self,
        _contract_address: String,
        _key: String,
    ) -> Result<FieldElement, Error> {
        self.sequencer
            .starknet_get_storage_at(
                ContractAddress(patricia_key!(_contract_address.as_str())),
                StorageKey(patricia_key!(_key.as_str())),
            )
            .await
            .map_err(|_| Error::from(KatanaApiError::ContractError))
    }
}

#[cfg(test)]
mod tests {
    use katana_core::sequencer::KatanaSequencer;

    use crate::{api::KatanaApiServer, KatanaRpc};

    #[tokio::test]
    async fn chain_id_is_ok() {
        let rpc = KatanaRpc::new(KatanaSequencer::new());
        let chain_id = rpc.chain_id().await.unwrap();
        assert_eq!(chain_id, "KATANA");
    }

    #[tokio::test]
    async fn nonce_is_ok() {
        let rpc = KatanaRpc::new(KatanaSequencer::new());
        let nonce = rpc.get_nonce("0xdead".to_string()).await.unwrap();
        assert_eq!(nonce, "0x0");
    }

    #[tokio::test]
    async fn block_number_is_ok() {
        let rpc = KatanaRpc::new(KatanaSequencer::new());
        let block_number = rpc.block_number().await.unwrap();
        assert_eq!(block_number, 0);
    }
}
