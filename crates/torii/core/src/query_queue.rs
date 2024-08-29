use std::collections::VecDeque;

use sqlx::{Executor, Pool, Sqlite};
use starknet::core::types::Felt;

#[derive(Debug, Clone)]
pub enum Argument {
    Null,
    Int(i64),
    Bool(bool),
    String(String),
    FieldElement(Felt),
}

#[derive(Debug, Clone)]
pub struct QueryQueue {
    pool: Pool<Sqlite>,
    pub queue: VecDeque<(String, Vec<Argument>)>,
}

impl QueryQueue {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        QueryQueue { pool, queue: VecDeque::new() }
    }

    pub fn enqueue<S: Into<String>>(&mut self, statement: S, arguments: Vec<Argument>) {
        self.queue.push_back((statement.into(), arguments));
    }

    pub fn push_front<S: Into<String>>(&mut self, statement: S, arguments: Vec<Argument>) {
        self.queue.push_front((statement.into(), arguments));
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

        Ok(total_affected)
    }
}
