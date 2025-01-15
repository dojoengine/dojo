use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use starknet::{core::types::Event, providers::Provider};
use torii_sqlite::types::ContractType;

use crate::engine::Processors;

pub type TaskId = u64;
type TaskPriority = usize;

#[derive(Debug)]
pub struct ParallelizedEvent {
    pub contract_type: ContractType,
    pub block_number: u64,
    pub block_timestamp: u64,
    pub event_id: String,
    pub event: Event,
}

pub struct TaskManager<P: Provider + Send + Sync + std::fmt::Debug + 'static> {
    tasks: BTreeMap<TaskPriority, HashMap<TaskId, Vec<ParallelizedEvent>>>,
    processors: Arc<Processors<P>>,
}

impl<P: Provider + Send + Sync + std::fmt::Debug + 'static> TaskManager<P> {
    pub fn new(processors: Arc<Processors<P>>) -> Self {
        Self { tasks: BTreeMap::new(), processors }
    }

    pub fn add_parallelized_event(&mut self, parallelized_event: ParallelizedEvent) -> TaskId {
        let event_key = parallelized_event.event.keys[0];
        let processor = self
            .processors
            .get_event_processor(parallelized_event.contract_type)
            .get(&event_key)
            .unwrap()
            .iter()
            .find(|p| p.validate(&parallelized_event.event))
            .unwrap();
        let priority = processor.task_priority();
        let task_id = processor.task_identifier(&parallelized_event.event);

        if task_id != 0 {
            self.tasks
                .entry(priority)
                .or_default()
                .entry(task_id)
                .or_default()
                .push(parallelized_event);
        }

        task_id
    }

    pub fn take_tasks(
        &mut self,
    ) -> BTreeMap<TaskPriority, HashMap<TaskId, Vec<(ContractType, ParallelizedEvent)>>> {
        std::mem::take(&mut self.tasks)
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}
