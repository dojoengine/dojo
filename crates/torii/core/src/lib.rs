use serde::Deserialize;
use sqlx::FromRow;

use crate::types::SQLFelt;

pub mod cache;
pub mod engine;
pub mod error;
pub mod model;
pub mod processors;
pub mod query_queue;
pub mod simple_broker;
pub mod sql;
pub mod types;
pub mod utils;

#[allow(dead_code)]
#[derive(FromRow, Deserialize)]
pub struct World {
    #[sqlx(try_from = "String")]
    world_address: SQLFelt,
    #[sqlx(try_from = "String")]
    world_class_hash: SQLFelt,
    #[sqlx(try_from = "String")]
    executor_address: SQLFelt,
    #[sqlx(try_from = "String")]
    executor_class_hash: SQLFelt,
}
