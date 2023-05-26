use anyhow::Result;
use async_trait::async_trait;
use starknet::core::types::FieldElement;

pub mod memory;
pub mod sql;

#[async_trait]
pub trait Storage {
    async fn head(&self) -> Result<u64>;
    async fn set_head(&mut self, head: u64) -> Result<()>;
    async fn create_component(&self, name: FieldElement, columns: Vec<FieldElement>) -> Result<()>;
    async fn set_entity(
        &self,
        component: FieldElement,
        partition: FieldElement,
        key: FieldElement,
        values: Vec<FieldElement>,
    ) -> Result<()>;
    async fn delete_entity(
        &self,
        component: FieldElement,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<()>;
    async fn entity(
        &self,
        component: FieldElement,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<Vec<FieldElement>>;
    async fn entities(
        &self,
        component: FieldElement,
        partition: FieldElement,
    ) -> Result<Vec<Vec<FieldElement>>>;
}
