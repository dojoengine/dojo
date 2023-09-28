use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::accounts::Account;
use katana_core::sequencer::KatanaSequencer;
use starknet::core::types::FieldElement;
use starknet_api::core::{ContractAddress, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::state::StorageKey;
use starknet_api::{patricia_key, stark_felt};

use crate::api::katana::{KatanaApiError, KatanaApiServer};

pub struct KatanaApi {
    sequencer: Arc<KatanaSequencer>,
}

impl KatanaApi {
    pub fn new(sequencer: Arc<KatanaSequencer>) -> Self {
        Self { sequencer }
    }
}

#[async_trait]
impl KatanaApiServer for KatanaApi {
    async fn generate_block(&self) -> Result<(), Error> {
        self.sequencer.block_producer().force_mine();
        Ok(())
    }

    async fn next_block_timestamp(&self) -> Result<u64, Error> {
        Ok(self.sequencer.backend().env.read().block.block_timestamp.0)
    }

    async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        self.sequencer
            .set_next_block_timestamp(timestamp)
            .await
            .map_err(|_| Error::from(KatanaApiError::FailedToChangeNextBlockTimestamp))
    }

    async fn increase_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        self.sequencer
            .increase_next_block_timestamp(timestamp)
            .await
            .map_err(|_| Error::from(KatanaApiError::FailedToChangeNextBlockTimestamp))
    }

    async fn predeployed_accounts(&self) -> Result<Vec<Account>, Error> {
        Ok(self.sequencer.backend().accounts.clone())
    }

    async fn set_storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        value: FieldElement,
    ) -> Result<(), Error> {
        self.sequencer
            .set_storage_at(
                ContractAddress(patricia_key!(contract_address)),
                StorageKey(patricia_key!(key)),
                stark_felt!(value),
            )
            .await
            .map_err(|_| Error::from(KatanaApiError::FailedToUpdateStorage))
    }
}
