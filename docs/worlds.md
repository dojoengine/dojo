# World

The World contract interface is as follows:

```rust
trait World {
    /// Gets the class hash of a registered component.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the component.
    ///
    /// # Returns
    ///
    /// * `ClassHash` - The class hash of the component.
    fn component(self: @ContractState, name: felt252) -> ClassHash;

    /// Registers a component in the world. If the component is already registered,
    /// the implementation will be updated.
    ///
    /// # Arguments
    ///
    /// * `class_hash` - The class hash of the component to be registered.
    fn register_component(ref self: ContractState, class_hash: ClassHash);

    /// Gets the class hash of a registered system.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the system.
    ///
    /// # Returns
    ///
    /// * `ClassHash` - The class hash of the system.
    fn system(self: @ContractState, name: felt252) -> ClassHash;

    /// Registers a system in the world. If the system is already registered,
    /// the implementation will be updated.
    ///
    /// # Arguments
    ///
    /// * `class_hash` - The class hash of the system to be registered.
    fn register_system(ref self: ContractState, class_hash: ClassHash);

    /// Issues an autoincremented id to the caller.
    ///
    /// # Returns
    ///
    /// * `usize` - The autoincremented id.
    fn uuid(ref self: ContractState) -> usize;

    /// Emits a custom event.
    ///
    /// # Arguments
    ///
    /// * `keys` - The keys of the event.
    /// * `values` - The data to be logged by the event.
    fn emit(self: @ContractState, keys: Span<felt252>, values: Span<felt252>);

    /// Executes a system with the given calldata.
    ///
    /// # Arguments
    ///
    /// * `system` - The name of the system to be executed.
    /// * `calldata` - The calldata to be passed to the system.
    ///
    /// # Returns
    ///
    /// * `Span<felt252>` - The result of the system execution.
    fn execute(ref self: ContractState, system: felt252, calldata: Span<felt252>) -> Span<felt252>;

    /// Gets the component value for an entity.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be retrieved.
    /// * `query` - The query to be used to find the entity.
    /// * `offset` - The offset of the component values.
    /// * `length` - The length of the component values.
    ///
    /// # Returns
    ///
    /// * `Span<felt252>` - The value of the component.
    fn entity(
        self: @ContractState, component: felt252, query: Query, offset: u8, length: usize
    ) -> Span<felt252>;

    /// Sets the component value for an entity.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be set.
    /// * `query` - The query to be used to find the entity.
    /// * `offset` - The offset of the component in the entity.
    /// * `value` - The value to be set.
    fn set_entity(ref self: ContractState, component: felt252, query: Query, offset: u8, value: Span<felt252>);

    /// Returns entity IDs and entities that contain the component state.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be retrieved.
    /// * `partition` - The partition to be retrieved.
    ///
    /// # Returns
    ///
    /// * `Span<felt252>` - The entity IDs.
    /// * `Span<Span<felt252>>` - The entities.
    fn entities(
        self: @ContractState, component: felt252, partition: felt252, length: usize
    ) -> (Span<felt252>, Span<Span<felt252>>);

    /// Sets the executor contract address.
    ///
    /// # Arguments
    ///
    /// * `contract_address` - The contract address of the executor.
    fn set_executor(ref self: ContractState, contract_address: ContractAddress);

    /// Gets the executor contract address.
    ///
    /// # Returns
    ///
    /// * `ContractAddress` - The address of the executor contract.
    fn executor(self: @ContractState) -> ContractAddress;

    /// Deletes a component from an entity.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be deleted.
    /// * `query` - The query to be used to find the entity.
    fn delete_entity(ref self: ContractState, component: felt252, query: Query);

    /// Gets the origin caller.
    ///
    /// # Returns
    ///
    /// * `felt252` - The origin caller.
    fn origin(self: @ContractState) -> ContractAddress;

    /// Checks if the provided account is an owner of the target.
    ///
    /// # Arguments
    ///
    /// * `account` - The account.
    /// * `target` - The target.
    ///
    /// # Returns
    ///
    /// * `bool` - True if the account is an owner of the target, false otherwise.
    fn is_owner(self: @ContractState, account: ContractAddress, target: felt252) -> bool;

    /// Grants ownership of the target to the account.
    /// Can only be called by an existing owner or the world admin.
    ///
    /// # Arguments
    ///
    /// * `account` - The account.
    /// * `target` - The target.
    fn grant_owner(ref self: ContractState, account: ContractAddress, target: felt252);

    /// Revokes owner permission to the system for the component.
    /// Can only be called by an existing owner or the world admin.
    ///
    /// # Arguments
    ///
    /// * `account` - The account.
    /// * `target` - The target.
    fn revoke_owner(ref self: ContractState, account: ContractAddress, target: felt252);

    /// Checks if the provided system is a writer of the component.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component.
    /// * `system` - The name of the system.
    ///
    /// # Returns
    ///
    /// * `bool` - True if the system is a writer of the component, false otherwise
    fn is_writer(self: @ContractState, component: felt252, system: felt252) -> bool;

    /// Grants writer permission to the system for the component.
    /// Can only be called by an existing component owner or the world admin.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component.
    /// * `system` - The name of the system.
    fn grant_writer(ref self: ContractState, component: felt252, system: felt252);

    /// Revokes writer permission to the system for the component.
    /// Can only be called by an existing component writer, owner or the world admin.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component.
    /// * `system` - The name of the system.
    fn revoke_writer(ref self: ContractState, component: felt252, system: felt252);
}
```

## Events

These are the events that could be emitted by the World contract.

```rust
/// When the World is initially deployed.
struct WorldSpawned {
    /// The address of the World contract.
    address: ContractAddress,
    /// The address of the account which deployed the World.
    caller: ContractAddress
}

/// When a component is registered to the World.
struct ComponentRegistered {
    /// The name of the component in ASCII.
    name: felt252,
    /// The class hash of the component.
    class_hash: ClassHash
}

/// When a system is registered to the World.
struct SystemRegistered {
    /// The name of the system in ASCII.
    name: felt252,
    /// The class hash of the system.
    class_hash: ClassHash
}

/// When a component value of an entity is set using World::set_entity().
struct StoreSetRecord {
    table_id: felt252,
    keys: Span<felt252>,
    offset: u8,
    value: Span<felt252>,
}

/// When a component value of an entity is deleted using World::delete_entity().
struct StoreDelRecord {
    table_id: felt252,
    keys: Span<felt252>,
}
```
