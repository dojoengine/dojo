use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::sequencer::KatanaSequencer;
use katana_executor::ExecutorFactory;
use katana_primitives::FieldElement;
use katana_rpc_api::dev::DevApiServer;
use katana_rpc_types::error::dev::DevApiError;

#[allow(missing_debug_implementations)]
pub struct DevApi<EF: ExecutorFactory> {
    sequencer: Arc<KatanaSequencer<EF>>,
}

impl<EF: ExecutorFactory> DevApi<EF> {
    pub fn new(sequencer: Arc<KatanaSequencer<EF>>) -> Self {
        Self { sequencer }
    }

    fn has_pending_transactions(&self) -> bool {
        if let Some(ref exec) = self.sequencer.pending_executor() {
            !exec.read().transactions().is_empty()
        } else {
            false
        }
    }

    pub fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), DevApiError> {
        if self.has_pending_transactions() {
            return Err(DevApiError::PendingTransactions);
        }

        let mut block_context_generator = self.sequencer.backend().block_context_generator.write();
        block_context_generator.next_block_start_time = timestamp;

        Ok(())
    }

    pub fn increase_next_block_timestamp(&self, offset: u64) -> Result<(), DevApiError> {
        if self.has_pending_transactions() {
            return Err(DevApiError::PendingTransactions);
        }

        let mut block_context_generator = self.sequencer.backend().block_context_generator.write();
        block_context_generator.block_timestamp_offset += offset as i64;

        Ok(())
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
        Ok(self.set_next_block_timestamp(timestamp)?)
    }

    async fn increase_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error> {
        Ok(self.increase_next_block_timestamp(timestamp)?)
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
