use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::backend::Backend;
use katana_core::service::block_producer::{BlockProducer, BlockProducerMode, PendingExecutor};
use katana_executor::ExecutorFactory;
use katana_primitives::genesis::constant::DEFAULT_FEE_TOKEN_ADDRESS;
use katana_primitives::{ContractAddress, Felt};
use katana_rpc_api::dev::DevApiServer;
use katana_rpc_types::account::Account;
use katana_rpc_types::error::dev::DevApiError;
use starknet::core::types::{BlockId, BlockTag, FunctionCall};
use starknet::macros::selector;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use url::Url;

#[allow(missing_debug_implementations)]
pub struct DevApi<EF: ExecutorFactory> {
    backend: Arc<Backend<EF>>,
    block_producer: Arc<BlockProducer<EF>>,
}

impl<EF: ExecutorFactory> DevApi<EF> {
    pub fn new(backend: Arc<Backend<EF>>, block_producer: Arc<BlockProducer<EF>>) -> Self {
        Self { backend, block_producer }
    }

    /// Returns the pending state if the sequencer is running in _interval_ mode. Otherwise `None`.
    fn pending_executor(&self) -> Option<PendingExecutor> {
        match &*self.block_producer.producer.read() {
            BlockProducerMode::Instant(_) => None,
            BlockProducerMode::Interval(producer) => Some(producer.executor()),
        }
    }

    fn has_pending_transactions(&self) -> bool {
        if let Some(ref exec) = self.pending_executor() {
            !exec.read().transactions().is_empty()
        } else {
            false
        }
    }

    pub fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), DevApiError> {
        if self.has_pending_transactions() {
            return Err(DevApiError::PendingTransactions);
        }

        let mut block_context_generator = self.backend.block_context_generator.write();
        block_context_generator.next_block_start_time = timestamp;

        Ok(())
    }

    pub fn increase_next_block_timestamp(&self, offset: u64) -> Result<(), DevApiError> {
        if self.has_pending_transactions() {
            return Err(DevApiError::PendingTransactions);
        }

        let mut block_context_generator = self.backend.block_context_generator.write();
        block_context_generator.block_timestamp_offset += offset as i64;
        Ok(())
    }
}

#[async_trait]
impl<EF: ExecutorFactory> DevApiServer for DevApi<EF> {
    async fn generate_block(&self) -> Result<(), Error> {
        self.block_producer.force_mine();
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
        _contract_address: Felt,
        _key: Felt,
        _value: Felt,
    ) -> Result<(), Error> {
        // self.sequencer
        //     .set_storage_at(contract_address.into(), key, value)
        //     .await
        //     .map_err(|_| Error::from(KatanaApiError::FailedToUpdateStorage))
        Ok(())
    }

    #[allow(deprecated)]
    async fn account_balance(&self, account_address: &str) -> Result<u128, Error> {
        // let account_address =
        //     address!("0x6b86e40118f29ebe393a75469b4d926c7a44c2e2681b6d319520b7c1156d114");
        let account_address = Felt::from_dec_str(account_address).unwrap();
        let account_address = ContractAddress::from(account_address);
        let url = Url::parse("http://localhost:5050").unwrap();
        let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(url)));
        let res = provider
            .call(
                FunctionCall {
                    contract_address: DEFAULT_FEE_TOKEN_ADDRESS.into(),
                    entry_point_selector: selector!("balanceOf"),
                    calldata: vec![account_address.into()],
                },
                BlockId::Tag(BlockTag::Latest),
            )
            .await;

        let balance: u128 = res.unwrap()[0].to_string().parse().unwrap();

        Ok(balance)
    }

    async fn fee_token(&self) -> Result<u64, Error> {
        Ok(1)
    }

    async fn mint(&self) -> Result<(), Error> {
        Ok(())
    }

    #[allow(deprecated)]
    async fn predeployed_accounts(&self) -> Result<Vec<Account>, Error> {
        Ok(self.backend.config.genesis.accounts().map(|e| Account::new(*e.0, e.1)).collect())
    }
}
