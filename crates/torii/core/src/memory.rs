use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use dojo_world::manifest::{Component, Manifest, System};
use starknet::core::types::FieldElement;

use super::State;

type Partition = FieldElement;
type Key = FieldElement;
type Entities = HashMap<Partition, HashMap<Key, Vec<FieldElement>>>;

#[derive(Default)]
pub struct InMemory {
    head: u64,
    components: Vec<Component>,
    systems: Vec<System>,
    components_to_entites: HashMap<String, Entities>,
}

#[async_trait]
impl State for InMemory {
    async fn load_from_manifest(&mut self, _manifest: Manifest) -> Result<()> {
        Ok(())
    }

    async fn head(&self) -> Result<u64> {
        Ok(self.head)
    }

    async fn set_head(&mut self, head: u64) -> Result<()> {
        self.head = head;
        Ok(())
    }

    async fn register_component(&mut self, component: Component) -> Result<()> {
        self.components.push(component);
        Ok(())
    }

    async fn register_system(&mut self, system: System) -> Result<()> {
        self.systems.push(system);
        Ok(())
    }

    async fn set_entity(
        &mut self,
        component: String,
        key: FieldElement,
        values: HashMap<String, FieldElement>,
    ) -> Result<()> {
        // if let Some(component_data) = self.components_to_entites.get_mut(&component) {
        //     let partition_data = component_data.entry(partition).or_insert_with(HashMap::new);
        //     partition_data.insert(key, values);
        // }
        Ok(())
    }

    async fn delete_entity(&mut self, component: String, key: FieldElement) -> Result<()> {
        if let Some(component_data) = self.components_to_entites.get_mut(&component) {
            if let Some(partition_data) = component_data.get_mut(&partition) {
                partition_data.remove(&key);
            }
        }
        Ok(())
    }

    async fn entity(&self, component: String, key: FieldElement) -> Result<Vec<FieldElement>> {
        if let Some(component_data) = self.components_to_entites.get(&component) {
            if let Some(partition_data) = component_data.get(&partition) {
                if let Some(entity) = partition_data.get(&key) {
                    return Ok(entity.clone());
                }
            }
        }
        Ok(vec![])
    }

    async fn entities(&self, component: String) -> Result<Vec<Vec<FieldElement>>> {
        let mut result = Vec::new();
        if let Some(component_data) = self.components_to_entites.get(&component) {
            if let Some(partition_data) = component_data.get(&partition) {
                for entity in partition_data.values() {
                    result.push(entity.clone());
                }
            }
        }
        Ok(result)
    }
}
