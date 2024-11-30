use std::future::IntoFuture;
use std::sync::Arc;

use anyhow::Result;
use futures::future::{self, BoxFuture};
use katana_core::backend::Backend;
use katana_core::service::block_producer::{BlockProducer, BlockProductionError};
use katana_core::service::messaging::{MessagingConfig, MessagingService, MessagingTask};
use katana_core::service::{BlockProductionTask, TransactionMiner};
use katana_executor::ExecutorFactory;
use katana_pool::{TransactionPool, TxPool};
use katana_tasks::{TaskHandle, TaskSpawner};
use tracing::error;

pub type SequencingFut = BoxFuture<'static, Result<()>>;

/// The sequencing stage is responsible for advancing the chain state.
#[allow(missing_debug_implementations)]
pub struct Sequencing<EF: ExecutorFactory> {
    pool: TxPool,
    backend: Arc<Backend<EF>>,
    task_spawner: TaskSpawner,
    block_producer: BlockProducer<EF>,
    messaging_config: Option<MessagingConfig>,
}

impl<EF: ExecutorFactory> Sequencing<EF> {
    pub fn new(
        pool: TxPool,
        backend: Arc<Backend<EF>>,
        task_spawner: TaskSpawner,
        block_producer: BlockProducer<EF>,
        messaging_config: Option<MessagingConfig>,
    ) -> Self {
        Self { pool, backend, task_spawner, block_producer, messaging_config }
    }

    async fn run_messaging(&self) -> Result<TaskHandle<()>> {
        if let Some(config) = &self.messaging_config {
            let config = config.clone();
            let pool = self.pool.clone();
            let backend = self.backend.clone();

            let service = MessagingService::new(config, pool, backend).await?;
            let task = MessagingTask::new(service);

            let handle = self.task_spawner.build_task().name("Messaging").spawn(task);
            Ok(handle)
        } else {
            let handle = self.task_spawner.build_task().spawn(future::pending::<()>());
            Ok(handle)
        }
    }

    fn run_block_production(&self) -> TaskHandle<Result<(), BlockProductionError>> {
        // Create a new transaction miner with a subscription to the pool's pending transactions.
        let miner = TransactionMiner::new(self.pool.pending_transactions());
        let block_producer = self.block_producer.clone();
        let service = BlockProductionTask::new(self.pool.clone(), miner, block_producer);
        self.task_spawner.build_task().name("Block production").spawn(service)
    }
}

impl<EF: ExecutorFactory> IntoFuture for Sequencing<EF> {
    type Output = Result<()>;
    type IntoFuture = SequencingFut;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            // Build the messaging and block production tasks.
            let messaging = self.run_messaging().await?;
            let block_production = self.run_block_production();

            // Neither of these tasks should complete as they are meant to be run forever,
            // but if either of them do complete, the sequencing stage should return.
            //
            // Select on the tasks completion to prevent the task from failing silently (if any).
            tokio::select! {
                res = messaging => {
                    error!(target: "sequencing", reason = ?res, "Messaging task finished unexpectedly.");
                },
                res = block_production => {
                    error!(target: "sequencing", reason = ?res, "Block production task finished unexpectedly.");
                }
            }

            Ok(())
        })
    }
}
