use dojo_world::manifest::{Component, System};
use starknet::core::types::FieldElement;

pub mod source;
pub mod storage;

use source::Source;
use storage::{Storage, StorageReader};

pub enum LocalWorldError {}

/// A local representation of a World.
#[derive(Debug)]
pub struct LocalWorld<S, R> {
    /// The adress of the World.
    address: FieldElement,
    /// The state of the World.
    state: S,
    /// The source from which the state may be loaded/synced.
    source: Option<R>,
}

impl<S, R> LocalWorld<S, R>
where
    S: Storage + StorageReader,
    R: Source,
{
    pub fn new(address: FieldElement, state: S) -> Self {
        Self { address, state, source: None }
    }

    pub async fn load(&mut self) -> Result<(), LocalWorldError> {
        // if self.source.is_none() {
        //     return Err(());
        // }
        // source.load(self.address, &mut self.state).await;
        Ok(())
    }

    pub fn address(&self) -> FieldElement {
        self.address
    }
}

impl<S, R> StorageReader for LocalWorld<S, R>
where
    S: StorageReader,
    R: Source,
{
    type Error = S::Error;

    fn component(&self, component: String) -> Result<Component, Self::Error> {
        self.state.component(component)
    }

    fn system(&self, system: String) -> Result<System, Self::Error> {
        self.state.system(system)
    }

    fn entities(
        &self,
        component: String,
        partition: starknet::core::types::FieldElement,
    ) -> Result<Vec<Vec<starknet::core::types::FieldElement>>, Self::Error> {
        self.state.entities(component, partition)
    }

    fn system_components(
        &self,
        system: String,
    ) -> Result<Vec<dojo_world::manifest::Dependency>, Self::Error> {
        self.state.system_components(system)
    }

    fn entity(
        &self,
        component: String,
        partition: starknet::core::types::FieldElement,
        key: starknet::core::types::FieldElement,
    ) -> Result<Vec<starknet::core::types::FieldElement>, Self::Error> {
        self.state.entity(component, partition, key)
    }

    fn executor(&self) -> Result<starknet::core::types::FieldElement, Self::Error> {
        self.state.executor()
    }
}
