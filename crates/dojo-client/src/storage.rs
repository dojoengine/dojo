use starknet::core::types::FieldElement;

use crate::types::{Component, System};

pub trait Storage {
    type Error;

    /// Insert a component by its name.
    fn set_component(&mut self, name: String, component: Component) -> Result<(), Self::Error>;
    /// Insert a system by its name.
    fn set_system(&mut self, name: String, system: System) -> Result<(), Self::Error>;
    /// Set the component value for an entity.
    fn set_entity(
        &mut self,
        component: String,
        partition: FieldElement,
        keys: Vec<FieldElement>,
        values: Vec<FieldElement>,
    ) -> Result<(), Self::Error>;
    /// Delete a component from an entity.
    fn delete_entity(
        &mut self,
        component: String,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<(), Self::Error>;
}

pub trait StorageReader {
    type Error;

    fn executor(&self) -> Result<FieldElement, Self::Error>;
    /// Get the component by its name
    fn component(&self, component: String) -> Result<Component, Self::Error>;
    /// Get the system by its name
    fn system(&self, system: String) -> Result<System, Self::Error>;

    /// Get the component value for an entity
    fn entity(
        &self,
        component: String,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<Vec<FieldElement>, Self::Error>;

    /// Get the entity IDs and entities that contain the component state
    fn entities(
        &self,
        component: String,
        partition: FieldElement,
    ) -> Result<Vec<Vec<FieldElement>>, Self::Error>;
}
