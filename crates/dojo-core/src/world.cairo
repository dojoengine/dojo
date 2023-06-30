use starknet::{ContractAddress, ClassHash, StorageAccess, StorageBaseAddress, SyscallResult};
use traits::{Into, TryInto};
use option::OptionTrait;

use dojo::interfaces::{IWorldDispatcher, IWorldDispatcherTrait};

#[derive(Copy, Drop, Serde)]
struct Context {
    world: IWorldDispatcher, // Dispatcher to the world contract
    origin: ContractAddress, // Address of the origin
    system: felt252, // Name of the calling system
    system_class_hash: ClassHash, // Class hash of the calling system
}

#[starknet::contract]
mod World {
    use array::{ArrayTrait, SpanTrait};
    use traits::Into;
    use option::OptionTrait;
    use box::BoxTrait;
    use serde::Serde;
    use starknet::{
        get_caller_address, get_contract_address, get_tx_info,
        contract_address::ContractAddressIntoFelt252, ClassHash, Zeroable,
        ContractAddress, syscalls::emit_event_syscall
    };

    use dojo::database;
    use dojo::database::query::{Query, QueryTrait};
    use dojo::interfaces::{
        IComponentLibraryDispatcher, IComponentDispatcherTrait, IExecutorDispatcher,
        IExecutorDispatcherTrait, ISystemLibraryDispatcher, ISystemDispatcherTrait,
        IWorldDispatcher
    };

    use super::Context;

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        WorldSpawned: WorldSpawned,
        ComponentRegistered: ComponentRegistered,
        SystemRegistered: SystemRegistered,
        StoreSetRecord: StoreSetRecord,
        StoreDelRecord: StoreDelRecord
    }

    #[derive(Drop, starknet::Event)]
    struct WorldSpawned {
        address: ContractAddress,
        caller: ContractAddress
    }

    #[derive(Drop, starknet::Event)]
    struct ComponentRegistered {
        name: felt252,
        class_hash: ClassHash
    }

    #[derive(Drop, starknet::Event)]
    struct SystemRegistered {
        name: felt252,
        class_hash: ClassHash
    }

    #[derive(Drop, starknet::Event)]
    struct StoreSetRecord {
        table_id: felt252,
        keys: Span<felt252>,
        offset: u8,
        value: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    struct StoreDelRecord {
        table_id: felt252,
        keys: Span<felt252>,
    }

    #[storage]
    struct Storage {
        executor_dispatcher: IExecutorDispatcher,
        components: LegacyMap::<felt252, ClassHash>,
        systems: LegacyMap::<felt252, ClassHash>,

        nonce: usize,

        owners: LegacyMap::<(felt252, ContractAddress), bool>,
        writers: LegacyMap::<(felt252, felt252), bool>,

        // Tracks the origin executor.
        origin: ContractAddress,

        // Tracks the calling systems name for auth purposes.
        call_stack_len: felt252,
        call_stack: LegacyMap::<felt252, felt252>,
    }

    #[constructor]
    fn constructor(ref self: ContractState, executor: ContractAddress) {        
        self.executor_dispatcher.write(IExecutorDispatcher { contract_address: executor });
        self.owners.write((0, starknet::get_tx_info().unbox().account_contract_address), bool::True(()));

        self.emit(
            WorldSpawned {
                address: get_contract_address(), 
                caller: get_tx_info().unbox().account_contract_address
            }
        );
    }

    /// Check if the provided account is an owner of the target
    ///
    /// # Returns
    ///
    /// * `bool` - True if the account is an owner of the target, false otherwise
    #[external(v0)]
    fn is_owner(self: @ContractState, account: ContractAddress, target: felt252) -> bool {
        self.owners.read((target, account))
    }

    /// Grants ownership of the target to the account
    /// Can only be called by an existing owner or the world admin
    #[external(v0)]
    fn grant_owner(ref self: ContractState, account: ContractAddress, target: felt252) {
        let caller = get_caller_address();
        assert(is_owner(@self, caller, target) || is_owner(@self, caller, 0), 'not owner');
        self.owners.write((target, account), bool::True(()));
    }

    /// Revokes owner permission to the system for the component
    /// Can only be called by an existing owner or the world admin
    #[external(v0)]
    fn revoke_owner(ref self: ContractState, account: ContractAddress, target: felt252) {
        assert(is_owner(@self, get_caller_address(), target) || is_owner(@self, get_caller_address(), 0), 'not owner');
        self.owners.write((target, account), bool::False(()));
    }

    /// Check if the provided system is an writer of the component
    ///
    /// # Returns
    ///
    /// * `bool` - True if the system is an writer of the component, false otherwise
    #[external(v0)]
    fn is_writer(self: @ContractState, component: felt252, system: felt252) -> bool {
        self.writers.read((component, system))
    }

    /// Grants writer permission to the system for the component
    /// Can only be called by an existing component owner or the world admin
    #[external(v0)]
    fn grant_writer(ref self: ContractState, component: felt252, system: felt252) {
        assert(is_owner(@self, get_caller_address(), component) || is_owner(@self, get_caller_address(), 0), 'not owner');
        self.writers.write((component, system), bool::True(()));
    }

    /// Revokes writer permission to the system for the component
    /// Can only be called by an existing component writer, owner or the world admin
    #[external(v0)]
    fn revoke_writer(ref self: ContractState, component: felt252, system: felt252) {
        assert(is_writer(@self, caller_system(@self), component) || is_owner(@self, get_caller_address(), component) || is_owner(@self, get_caller_address(), 0), 'not owner');
        self.writers.write((component, system), bool::False(()));
    }

    /// Register a component in the world. If the component is already registered,
    /// the implementation will be updated.
    ///
    /// # Arguments
    ///
    /// * `class_hash` - The class hash of the component to be registered
    #[external(v0)]
    fn register_component(ref self: ContractState, class_hash: ClassHash) {
        let name = IComponentLibraryDispatcher { class_hash: class_hash }.name();

        // If component is already registered, validate permission to update.
        if self.components.read(name).is_non_zero() {
            assert(is_owner(@self, get_caller_address(), name), 'only owner can update');
        }

        self.components.write(name, class_hash);
        self.emit(ComponentRegistered{ name, class_hash });
    }

    /// Get the class hash of a registered component
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the component
    ///
    /// # Returns
    ///
    /// * `ClassHash` - The class hash of the component
    #[external(v0)]
    fn component(self: @ContractState, name: felt252) -> ClassHash {
        self.components.read(name)
    }

    /// Register a system in the world. If the system is already registered,
    /// the implementation will be updated.
    ///
    /// # Arguments
    ///
    /// * `class_hash` - The class hash of the system to be registered
    #[external(v0)]
    fn register_system(ref self: ContractState, class_hash: ClassHash) {
        let name = ISystemLibraryDispatcher { class_hash: class_hash }.name();

        // If system is already registered, validate permission to update.
        if self.systems.read(name).is_non_zero() {
            assert(is_owner(@self, get_caller_address(), name), 'only owner can update');
        }

        self.systems.write(name, class_hash);
        self.emit(SystemRegistered{ name, class_hash });
    }

    /// Get the class hash of a registered system
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the system
    ///
    /// # Returns
    ///
    /// * `ClassHash` - The class hash of the system
    #[external(v0)]
    fn system(self: @ContractState, name: felt252) -> ClassHash {
        self.systems.read(name)
    }

    /// Execute a system with the given calldata
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the system to be executed
    /// * `calldata` - The calldata to be passed to the system
    ///
    /// # Returns
    ///
    /// * `Span<felt252>` - The result of the system execution
    #[external(v0)]
    fn execute(
        ref self: ContractState, system: felt252, calldata: Span<felt252>
    ) -> Span<felt252> {
        let stack_len = self.call_stack_len.read();
        self.call_stack.write(stack_len, system);
        self.call_stack_len.write(stack_len + 1);

        // Get the class hash of the system to be executed
        let system_class_hash = self.systems.read(system);

        let mut origin = self.origin.read();
        if origin.is_zero() {
            origin = get_caller_address();
            self.origin.write(origin);
        }

        // Call the system via executor
        let res = self
            .executor_dispatcher
            .read()
            .execute(Context {
                world: IWorldDispatcher { contract_address: get_contract_address() },
                origin: self.origin.read(),
                system,
                system_class_hash,
            }, calldata);

        self.call_stack.write(stack_len, 0);
        self.call_stack_len.write(stack_len);

        res
    }

    fn caller_system(self: @ContractState) -> felt252 {
        self.call_stack.read(self.call_stack_len.read() - 1)
    }

    /// Issue an autoincremented id to the caller.
    ///
    /// # Returns
    ///
    /// * `usize` - The autoincremented id
    #[external(v0)]
    fn uuid(ref self: ContractState) -> usize {
        let current = self.nonce.read();
        self.nonce.write(current + 1);
        current
    }

    /// Emits a custom event
    ///  
    /// # Arguments
    ///
    /// * `keys` - The keys of the event 
    /// * `values` - The data to be logged by the event
    #[external(v0)]
    fn emit(self: @ContractState, keys: Span<felt252>, values: Span<felt252>) {
        // Assert can only be called through the executor
        // This is to prevent system from writing to storage directly
        assert(
            get_caller_address() == self.executor_dispatcher.read().contract_address,
            'must be called thru executor'
        );

        emit_event_syscall(keys, values).unwrap_syscall();
    }

    /// Set the component value for an entity
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be set
    /// * `query` - The query to be used to find the entity
    /// * `offset` - The offset of the component in the entity
    /// * `value` - The value to be set
    #[external(v0)]
    fn set_entity(
        ref self: ContractState,
        component: felt252,
        query: Query,
        offset: u8,
        value: Span<felt252>
    ) {
        assert_can_write(@self, component);

        let table_id = query.table(component);
        let keys = query.keys();
        let component_class_hash = self.components.read(component);
        database::set(component_class_hash, table_id, query, offset, value);

        self.emit(StoreSetRecord{ table_id, keys, offset, value });
    }

    /// Delete a component from an entity
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be deleted
    /// * `query` - The query to be used to find the entity
    #[external(v0)]
    fn delete_entity(ref self: ContractState, component: felt252, query: Query) {
        assert_can_write(@self, component);

        let table_id = query.table(component);
        let keys = query.keys();
        let component_class_hash = self.components.read(component);
        database::del(component_class_hash, component.into(), query);

        self.emit(StoreDelRecord{ table_id, keys });
    }

    /// Get the component value for an entity
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be retrieved
    /// * `query` - The query to be used to find the entity
    /// * `offset` - The offset of the component values
    /// * `length` - The length of the component values
    ///
    /// # Returns
    ///
    /// * `Span<felt252>` - The value of the component
    #[external(v0)]
    fn entity(self: @ContractState, component: felt252, query: Query, offset: u8, length: usize) -> Span<felt252> {
        let class_hash = self.components.read(component);
        let table = query.table(component);
        match database::get(class_hash, table, query, offset, length) {
            Option::Some(res) => res,
            Option::None(_) => {
                ArrayTrait::new().span()
            }
        }
    }

    /// Returns entity IDs and entities that contain the component state.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be retrieved
    /// * `partition` - The partition to be retrieved
    ///
    /// # Returns
    ///
    /// * `Span<felt252>` - The entity IDs
    /// * `Span<Span<felt252>>` - The entities
    #[external(v0)]
    fn entities(self: @ContractState, component: felt252, partition: felt252) -> (Span<felt252>, Span<Span<felt252>>) {
        let class_hash = self.components.read(component);
        database::all(class_hash, component.into(), partition)
    }

    /// Set the executor contract address
    ///
    /// # Arguments
    ///
    /// * `contract_address` - The contract address of the executor
    #[external(v0)]
    fn set_executor(ref self: ContractState, contract_address: ContractAddress) {
        // Only owner can set executor
        assert(is_owner(@self, get_caller_address(), 0), 'only owner can set executor');
        self.executor_dispatcher.write(IExecutorDispatcher { contract_address: contract_address });
    }

    /// Get the executor contract address
    #[external(v0)]
    fn executor(self: @ContractState) -> ContractAddress {
        self.executor_dispatcher.read().contract_address
    }

    /// Assert that the current caller can write to the component
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component being written to
    fn assert_can_write(self: @ContractState, component: felt252) {
        assert(
            get_caller_address() == self.executor_dispatcher.read().contract_address,
            'must be called thru executor'
        );

        assert(
            is_writer(self, caller_system(self), component) ||
            is_owner(self, get_tx_info().unbox().account_contract_address, component) ||
            is_owner(self, get_tx_info().unbox().account_contract_address, 0), 'not writer');
    }
}

#[system]
mod library_call {
    fn execute(
        class_hash: starknet::ClassHash, entrypoint: felt252, calladata: Span<felt252>
    ) -> Span<felt252> {
        starknet::syscalls::library_call_syscall(class_hash, entrypoint, calladata).unwrap_syscall()
    }
}
