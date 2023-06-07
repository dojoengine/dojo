use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use dojo_world::manifest::{Component, Manifest, System};
use serde::Deserialize;
use sqlx::FromRow;
use starknet::core::types::FieldElement;

use crate::types::SQLFieldElement;

// pub mod memory;
pub mod sql;

#[derive(FromRow, Deserialize)]
pub struct World {
    #[sqlx(try_from = "String")]
    world_address: SQLFieldElement,
    #[sqlx(try_from = "String")]
    world_class_hash: SQLFieldElement,
    #[sqlx(try_from = "String")]
    executor_address: SQLFieldElement,
    #[sqlx(try_from = "String")]
    executor_class_hash: SQLFieldElement,
}

#[async_trait]
pub trait State {
    async fn load_from_manifest(&mut self, manifest: Manifest) -> Result<()>;
    async fn head(&self) -> Result<u64>;
    async fn set_head(&mut self, head: u64) -> Result<()>;
    async fn world(&self) -> Result<World>;
    async fn set_world(&mut self, world: World) -> Result<()>;
    async fn register_component(&mut self, component: Component) -> Result<()>;
    async fn register_system(&mut self, system: System) -> Result<()>;
    async fn set_entity(
        &mut self,
        component: String,
        partition: FieldElement,
        key: FieldElement,
        values: HashMap<String, FieldElement>,
    ) -> Result<()>;
    async fn delete_entity(
        &mut self,
        component: String,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<()>;
    async fn entity(
        &self,
        component: String,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<Vec<FieldElement>>;
    async fn entities(
        &self,
        component: String,
        partition: FieldElement,
    ) -> Result<Vec<Vec<FieldElement>>>;
}
