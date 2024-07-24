use std::sync::Arc;

use katana_executor::ExecutorFactory;
use katana_provider::BlockchainProvider;

use crate::backend::config::StarknetConfig;
use crate::backend::storage::Database;
use crate::backend::Backend;
use crate::pool::TransactionPool;
use crate::service::block_producer::{BlockProducer, BlockProducerMode, PendingExecutor};
#[cfg(feature = "messaging")]
use crate::service::messaging::MessagingConfig;
#[cfg(feature = "messaging")]
use crate::service::messaging::MessagingService;
use crate::service::{NodeService, TransactionMiner};

#[derive(Debug, Default)]
pub struct SequencerConfig {
    pub block_time: Option<u64>,
    pub no_mining: bool,
    #[cfg(feature = "messaging")]
    pub messaging: Option<MessagingConfig>,
}

#[allow(missing_debug_implementations)]
pub struct KatanaSequencer<EF: ExecutorFactory> {
    config: SequencerConfig,
    pool: Arc<TransactionPool>,
    backend: Arc<Backend<EF>>,
    block_producer: Arc<BlockProducer<EF>>,
}

impl<EF: ExecutorFactory> KatanaSequencer<EF> {
    pub async fn new(
        executor_factory: EF,
        config: SequencerConfig,
        starknet_config: StarknetConfig,
    ) -> anyhow::Result<Self> {
        let executor_factory = Arc::new(executor_factory);
        let backend = Arc::new(Backend::new(executor_factory.clone(), starknet_config).await);

        let pool = Arc::new(TransactionPool::new());
        let miner = TransactionMiner::new(pool.add_listener());

        let block_producer = if config.block_time.is_some() || config.no_mining {
            if let Some(interval) = config.block_time {
                BlockProducer::interval(Arc::clone(&backend), interval)
            } else {
                BlockProducer::on_demand(Arc::clone(&backend))
            }
        } else {
            BlockProducer::instant(Arc::clone(&backend))
        };

        #[cfg(feature = "messaging")]
        let messaging = if let Some(config) = config.messaging.clone() {
            MessagingService::new(config, Arc::clone(&pool), Arc::clone(&backend)).await.ok()
        } else {
            None
        };

        let block_producer = Arc::new(block_producer);

        tokio::spawn(NodeService::new(
            Arc::clone(&pool),
            miner,
            block_producer.clone(),
            #[cfg(feature = "messaging")]
            messaging,
        ));

        Ok(Self { pool, config, backend, block_producer })
    }

    /// Returns the pending state if the sequencer is running in _interval_ mode. Otherwise `None`.
    pub fn pending_executor(&self) -> Option<PendingExecutor> {
        match &*self.block_producer.inner.read() {
            BlockProducerMode::Instant(_) => None,
            BlockProducerMode::Interval(producer) => Some(producer.executor()),
        }
    }

    pub fn block_producer(&self) -> &BlockProducer<EF> {
        &self.block_producer
    }

    pub fn backend(&self) -> &Backend<EF> {
        &self.backend
    }

    pub fn pool(&self) -> &Arc<TransactionPool> {
        &self.pool
    }

    pub fn config(&self) -> &SequencerConfig {
        &self.config
    }

    pub fn provider(&self) -> &BlockchainProvider<Box<dyn Database>> {
        &self.backend.blockchain.provider()
    }

    // pub async fn set_storage_at(
    //     &self,
    //     contract_address: ContractAddress,
    //     storage_key: StorageKey,
    //     value: StorageValue,
    // ) -> Result<(), SequencerError> { if let Some(ref pending) = self.pending_state() {
    //   StateWriter::set_storage(&pending.state, contract_address, storage_key, value)?; } Ok(())
    // }
}

#[cfg(test)]
mod tests {
    use katana_executor::implementation::noop::NoopExecutorFactory;
    use katana_provider::traits::block::BlockNumberProvider;

    use super::{KatanaSequencer, SequencerConfig};
    use crate::backend::config::StarknetConfig;

    #[tokio::test]
    async fn init_interval_block_producer_with_correct_block_env() {
        let executor_factory = NoopExecutorFactory::default();

        let sequencer = KatanaSequencer::new(
            executor_factory,
            SequencerConfig { no_mining: true, ..Default::default() },
            StarknetConfig::default(),
        )
        .await
        .unwrap();

        let provider = sequencer.backend.blockchain.provider();

        let latest_num = provider.latest_number().unwrap();
        let producer_block_env = sequencer.pending_executor().unwrap().read().block_env();

        assert_eq!(
            producer_block_env.number,
            latest_num + 1,
            "Pending block number should be latest block number + 1"
        );
    }
}
