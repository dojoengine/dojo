use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use anyhow::Result;
use dojo_world::contracts::WorldContractReader;
use futures_util::future::try_join_all;
use starknet::core::types::Event;
use starknet::providers::Provider;
use tokio::sync::Semaphore;
use torii_sqlite::types::ContractType;
use torii_sqlite::Sql;
use tracing::{debug, error};

use crate::engine::Processors;
use crate::processors::EventProcessorConfig;

pub const TASK_ID_SEQUENTIAL: TaskId = 0;

const LOG_TARGET: &str = "torii_indexer::task_manager";

pub type TaskId = u64;
pub type TaskPriority = usize;

#[derive(Debug)]
pub struct ParallelizedEvent {
    pub contract_type: ContractType,
    pub block_number: u64,
    pub block_timestamp: u64,
    pub event_id: String,
    pub event: Event,
}

pub struct TaskManager<P: Provider + Send + Sync + std::fmt::Debug + 'static> {
    db: Sql,
    world: Arc<WorldContractReader<P>>,
    tasks: BTreeMap<TaskPriority, HashMap<TaskId, Vec<ParallelizedEvent>>>,
    processors: Arc<Processors<P>>,
    max_concurrent_tasks: usize,
    event_processor_config: EventProcessorConfig,
}

impl<P: Provider + Send + Sync + std::fmt::Debug + 'static> TaskManager<P> {
    pub fn new(
        db: Sql,
        world: Arc<WorldContractReader<P>>,
        processors: Arc<Processors<P>>,
        max_concurrent_tasks: usize,
        event_processor_config: EventProcessorConfig,
    ) -> Self {
        Self {
            db,
            world,
            tasks: BTreeMap::new(),
            processors,
            max_concurrent_tasks,
            event_processor_config,
        }
    }

    pub fn add_parallelized_event(
        &mut self,
        priority: TaskPriority,
        task_identifier: TaskId,
        parallelized_event: ParallelizedEvent,
    ) {
        self.tasks
            .entry(priority)
            .or_default()
            .entry(task_identifier)
            .or_default()
            .push(parallelized_event);
    }

    pub async fn process_tasks(&mut self) -> Result<()> {
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_tasks));

        // Process each priority level sequentially
        for (priority, task_group) in std::mem::take(&mut self.tasks) {
            let mut handles = Vec::new();

            // Process all tasks within this priority level concurrently
            for (task_id, events) in task_group {
                let db = self.db.clone();
                let world = self.world.clone();
                let semaphore = semaphore.clone();
                let processors = self.processors.clone();
                let event_processor_config = self.event_processor_config.clone();

                handles.push(tokio::spawn(async move {
                    let _permit = semaphore.acquire().await?;
                    let mut local_db = db.clone();

                    // Process all events for this task sequentially
                    for ParallelizedEvent {
                        contract_type,
                        event,
                        block_number,
                        block_timestamp,
                        event_id,
                    } in events
                    {
                        let contract_processors = processors.get_event_processor(contract_type);
                        if let Some(processors) = contract_processors.get(&event.keys[0]) {
                            let processor = processors
                                .iter()
                                .find(|p| p.validate(&event))
                                .expect("Must find at least one processor for the event");

                            debug!(
                                target: LOG_TARGET,
                                event_name = processor.event_key(),
                                task_id = %task_id,
                                priority = %priority,
                                "Processing parallelized event."
                            );

                            if let Err(e) = processor
                                .process(
                                    &world,
                                    &mut local_db,
                                    block_number,
                                    block_timestamp,
                                    &event_id,
                                    &event,
                                    &event_processor_config,
                                )
                                .await
                            {
                                error!(
                                    target: LOG_TARGET,
                                    event_name = processor.event_key(),
                                    error = %e,
                                    task_id = %task_id,
                                    priority = %priority,
                                    "Processing parallelized event."
                                );
                            }
                        }
                    }

                    Ok::<_, anyhow::Error>(())
                }));
            }

            // Wait for all tasks in this priority level to complete before moving to next priority
            try_join_all(handles).await?;
        }

        Ok(())
    }
}
