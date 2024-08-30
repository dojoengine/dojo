use std::collections::VecDeque;

use sqlx::{Executor, Pool, Sqlite};
use starknet::core::types::Felt;

use crate::simple_broker::SimpleBroker;
use crate::types::{
    Entity as EntityUpdated, Event as EventEmitted, EventMessage as EventMessageUpdated,
    Model as ModelRegistered,
};

#[derive(Debug, Clone)]
pub enum Argument {
    Null,
    Int(i64),
    Bool(bool),
    String(String),
    FieldElement(Felt),
}

#[derive(Debug, Clone)]
pub enum BrokerMessage {
    ModelRegistered(ModelRegistered),
    EntityUpdated(EntityUpdated),
    EventMessageUpdated(EventMessageUpdated),
    EventEmitted(EventEmitted),
}

#[derive(Debug, Clone)]
pub struct QueryQueue {
    pool: Pool<Sqlite>,
    pub queue: VecDeque<(String, Vec<Argument>)>,
    // publishes that are related to queries in the queue, they should be sent
    // after the queries are executed
    pub publish_queue: VecDeque<BrokerMessage>,
}

impl QueryQueue {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        QueryQueue { pool, queue: VecDeque::new(), publish_queue: VecDeque::new() }
    }

    pub fn enqueue<S: Into<String>>(&mut self, statement: S, arguments: Vec<Argument>) {
        self.queue.push_back((statement.into(), arguments));
    }

    pub fn push_front<S: Into<String>>(&mut self, statement: S, arguments: Vec<Argument>) {
        self.queue.push_front((statement.into(), arguments));
    }

    pub fn push_publish(&mut self, value: BrokerMessage) {
        self.publish_queue.push_back(value);
    }

    pub async fn execute_all(&mut self) -> sqlx::Result<u64> {
        let mut total_affected = 0_u64;
        let mut tx = self.pool.begin().await?;

        while let Some((statement, arguments)) = self.queue.pop_front() {
            let mut query = sqlx::query(&statement);

            for arg in &arguments {
                query = match arg {
                    Argument::Null => query.bind(None::<String>),
                    Argument::Int(integer) => query.bind(integer),
                    Argument::Bool(bool) => query.bind(bool),
                    Argument::String(string) => query.bind(string),
                    Argument::FieldElement(felt) => query.bind(format!("{:#x}", felt)),
                }
            }

            total_affected += tx.execute(query).await?.rows_affected();
        }

        tx.commit().await?;

        while let Some(message) = self.publish_queue.pop_front() {
            match message {
                BrokerMessage::ModelRegistered(model) => SimpleBroker::publish(model),
                BrokerMessage::EntityUpdated(entity) => SimpleBroker::publish(entity),
                BrokerMessage::EventMessageUpdated(event) => SimpleBroker::publish(event),
                BrokerMessage::EventEmitted(event) => SimpleBroker::publish(event),
            }
        }

        Ok(total_affected)
    }
}
