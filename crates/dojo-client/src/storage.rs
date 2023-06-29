use dojo_world::manifest::{Component, Dependency, System};
use starknet::core::types::FieldElement;

/// Resposible for storage operations where the World's state persistence is managed.
///
/// [`Storage`] defines a mutable interface to the storage.
pub trait Storage {
    type Error;

    /// Register a component.
    fn register_component(&mut self, component: Component) -> Result<(), Self::Error>;

    /// Register a system.
    fn register_system(&mut self, system: System) -> Result<(), Self::Error>;

    /// Set the executor contract address.
    fn set_executor(&mut self, executor: FieldElement) -> Result<(), Self::Error>;

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

/// [`StorageReader`] defines a immutable interface or a reference to the storage.
pub trait StorageReader {
    type Error;

    /// Get the executor contract address.
    fn executor(&self) -> Result<FieldElement, Self::Error>;

    /// Get the component by its name
    fn component(&self, component: String) -> Result<Component, Self::Error>;

    /// Get the system by its name
    fn system(&self, system: String) -> Result<System, Self::Error>;

    /// Get the component dependencies of a system
    fn system_components(&self, system: String) -> Result<Vec<Dependency>, Self::Error>;

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
