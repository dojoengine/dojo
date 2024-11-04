use std::sync::Arc;

use anyhow::Result;
use futures::future;
use katana_core::backend::Backend;
use katana_core::service::block_producer::{BlockProducer, BlockProductionError};
use katana_core::service::messaging::{MessagingConfig, MessagingService, MessagingTask};
use katana_core::service::{BlockProductionTask, TransactionMiner};
use katana_executor::ExecutorFactory;
use katana_pool::{TransactionPool, TxPool};
use katana_tasks::{TaskHandle, TaskSpawner};
use tracing::error;

use super::{StageId, StageResult};
use crate::Stage;

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
        let pool = self.pool.clone();
        let miner = TransactionMiner::new(pool.pending_transactions(), pool.add_listener());
        let block_producer = self.block_producer.clone();

        let service = BlockProductionTask::new(pool, miner, block_producer);
        self.task_spawner.build_task().name("Block production").spawn(service)
    }
}

#[async_trait::async_trait]
impl<EF: ExecutorFactory> Stage for Sequencing<EF> {
    fn id(&self) -> StageId {
        StageId::Sequencing
    }

    #[tracing::instrument(skip(self), name = "Stage", fields(id = %self.id()))]
    async fn execute(&mut self) -> StageResult {
        // Build the messaging and block production tasks.
        let messaging = self.run_messaging().await?;
        let block_production = self.run_block_production();

        // Neither of these tasks should complete as they are meant to be run forever,
        // but if either of them do complete, the sequencing stage should return.
        //
        // Select on the tasks completion to prevent the task from failing silently (if any).
        tokio::select! {
            res = messaging => {
                error!(target: "pipeline", reason = ?res, "Messaging task finished unexpectedly.");
            },
            res = block_production => {
                error!(target: "pipeline", reason = ?res, "Block production task finished unexpectedly.");
            }
        }

        Ok(())
    }
}
