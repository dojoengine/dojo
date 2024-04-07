use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::sequencer::KatanaSequencer;
use katana_executor::ExecutorFactory;
use katana_primitives::FieldElement;
use katana_rpc_api::dev::DevApiServer;
use katana_rpc_types::error::katana::KatanaApiError;

pub struct DevApi<EF: ExecutorFactory> {
    sequencer: Arc<KatanaSequencer<EF>>,
}

impl<EF: ExecutorFactory> DevApi<EF> {
    pub fn new(sequencer: Arc<KatanaSequencer<EF>>) -> Self {
        Self { sequencer }
    }
}

#[async_trait]
impl<EF: ExecutorFactory> DevApiServer for DevApi<EF> {
    async fn generate_block(&self) -> Result<(), Error> {
        self.sequencer.block_producer().force_mine();
        Ok(())
    }

    async fn next_block_timestamp(&self) -> Result<(), Error> {
        // Ok(self.sequencer.backend().env.read().block.block_timestamp.0)
        Ok(())
    }

    async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        self.sequencer
            .set_next_block_timestamp(timestamp)
            .map_err(|_| Error::from(KatanaApiError::FailedToChangeNextBlockTimestamp))
    }

    async fn increase_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        self.sequencer
            .increase_next_block_timestamp(timestamp)
            .map_err(|_| Error::from(KatanaApiError::FailedToChangeNextBlockTimestamp))
    }

    async fn set_storage_at(
        &self,
        _contract_address: FieldElement,
        _key: FieldElement,
        _value: FieldElement,
    ) -> Result<(), Error> {
        // self.sequencer
        //     .set_storage_at(contract_address.into(), key, value)
        //     .await
        //     .map_err(|_| Error::from(KatanaApiError::FailedToUpdateStorage))
        Ok(())
    }
}
