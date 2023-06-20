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
    async fn load_from_manifest(&self, manifest: Manifest) -> Result<()>;
    async fn head(&self) -> Result<u64>;
    async fn set_head(&self, head: u64) -> Result<()>;
    async fn world(&self) -> Result<World>;
    async fn set_world(&self, world: World) -> Result<()>;
    async fn register_component(&self, component: Component) -> Result<()>;
    async fn register_system(&self, system: System) -> Result<()>;
    async fn set_entity(
        &self,
        component: String,
        partition: FieldElement,
        keys: Vec<FieldElement>,
        values: Vec<FieldElement>,
    ) -> Result<()>;
    async fn delete_entity(
        &self,
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
