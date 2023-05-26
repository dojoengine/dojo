use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use starknet::core::types::FieldElement;
use tokio::sync::RwLock;

use super::Storage;

type Partition = FieldElement;
type Component = FieldElement;
type Key = FieldElement;
type Entities = HashMap<Partition, HashMap<Key, Vec<FieldElement>>>;
type Components = HashMap<Component, Entities>;

#[derive(Default)]
pub struct MemoryStorage {
    head: u64,
    data: Arc<RwLock<Components>>,
}

#[async_trait]
impl Storage for MemoryStorage {
    async fn head(&self) -> Result<u64> {
        Ok(self.head)
    }

    async fn set_head(&mut self, head: u64) -> Result<()> {
        self.head = head;
        Ok(())
    }

    async fn create_component(
        &self,
        name: FieldElement,
        _columns: Vec<FieldElement>,
    ) -> Result<()> {
        let mut data = self.data.write().await;
        data.entry(name).or_insert_with(HashMap::new);
        Ok(())
    }

    async fn set_entity(
        &self,
        component: FieldElement,
        partition: FieldElement,
        key: FieldElement,
        values: Vec<FieldElement>,
    ) -> Result<()> {
        let mut data = self.data.write().await;
        if let Some(component_data) = data.get_mut(&component) {
            let partition_data = component_data.entry(partition).or_insert_with(HashMap::new);
            partition_data.insert(key, values);
        }
        Ok(())
    }

    async fn delete_entity(
        &self,
        component: FieldElement,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<()> {
        let mut data = self.data.write().await;
        if let Some(component_data) = data.get_mut(&component) {
            if let Some(partition_data) = component_data.get_mut(&partition) {
                partition_data.remove(&key);
            }
        }
        Ok(())
    }

    async fn entity(
        &self,
        component: FieldElement,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<Vec<FieldElement>> {
        let data = self.data.read().await;
        if let Some(component_data) = data.get(&component) {
            if let Some(partition_data) = component_data.get(&partition) {
                if let Some(entity) = partition_data.get(&key) {
                    return Ok(entity.clone());
                }
            }
        }
        Ok(vec![])
    }

    async fn entities(
        &self,
        component: FieldElement,
        partition: FieldElement,
    ) -> Result<Vec<Vec<FieldElement>>> {
        let mut result = Vec::new();
        let data = self.data.read().await;
        if let Some(component_data) = data.get(&component) {
            if let Some(partition_data) = component_data.get(&partition) {
                for entity in partition_data.values() {
                    result.push(entity.clone());
                }
            }
        }
        Ok(result)
    }
}
