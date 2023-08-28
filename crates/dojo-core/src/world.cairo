use starknet::{ContractAddress, ClassHash, StorageBaseAddress, SyscallResult};
use traits::{Into, TryInto};
use option::OptionTrait;

#[derive(Copy, Drop, Serde)]
struct Context {
    world: IWorldDispatcher, // Dispatcher to the world contract
    origin: ContractAddress, // Address of the origin
    system: felt252, // Name of the calling system
    system_class_hash: ClassHash, // Class hash of the calling system
}

#[starknet::interface]
trait IWorld<T> {
    fn component(self: @T, name: felt252) -> ClassHash;
    fn register_component(ref self: T, class_hash: ClassHash);
    fn system(self: @T, name: felt252) -> ClassHash;
    fn register_system(ref self: T, class_hash: ClassHash);
    fn uuid(ref self: T) -> usize;
    fn emit(self: @T, keys: Array<felt252>, values: Span<felt252>);
    fn execute(ref self: T, system: felt252, calldata: Array<felt252>) -> Span<felt252>;
    fn entity(
        self: @T, component: felt252, keys: Span<felt252>, offset: u8, length: usize
    ) -> Span<felt252>;
    fn set_entity(
        ref self: T, component: felt252, keys: Span<felt252>, offset: u8, value: Span<felt252>
    );
    fn entities(
        self: @T, component: felt252, index: felt252, length: usize
    ) -> (Span<felt252>, Span<Span<felt252>>);
    fn set_executor(ref self: T, contract_address: ContractAddress);
    fn executor(self: @T) -> ContractAddress;
    fn delete_entity(ref self: T, component: felt252, keys: Span<felt252>);
    fn origin(self: @T) -> ContractAddress;
    fn caller_system(self: @T) -> felt252;

    fn is_owner(self: @T, address: ContractAddress, target: felt252) -> bool;
    fn grant_owner(ref self: T, address: ContractAddress, target: felt252);
    fn revoke_owner(ref self: T, address: ContractAddress, target: felt252);

    fn is_writer(self: @T, component: felt252, system: felt252) -> bool;
    fn grant_writer(ref self: T, component: felt252, system: felt252);
    fn revoke_writer(ref self: T, component: felt252, system: felt252);
    fn register_middleware(&self, middleware: Middleware);
    fn unregister_middleware(&self, middleware: Middleware);
    fn get_middleware(&self) -> Array<Middleware>;
}

#[starknet::contract]
mod world {
    use array::{ArrayTrait, SpanTrait};
    use traits::Into;
    use option::OptionTrait;
    use box::BoxTrait;
    use serde::Serde;
    use starknet::{
        get_caller_address, get_contract_address, get_tx_info,
        contract_address::ContractAddressIntoFelt252, ClassHash, Zeroable, ContractAddress,
        syscalls::emit_event_syscall
    };

    use dojo::database;
    use dojo::executor::{IExecutorDispatcher, IExecutorDispatcherTrait};
    use dojo::traits::{INamedLibraryDispatcher, INamedDispatcherTrait, };
    use dojo::world::{IWorldDispatcher, IWorld};

    use super::Context;

    const NAME_ENTRYPOINT: felt252 =
        0x0361458367e696363fbcc70777d07ebbd2394e89fd0adcaf147faccd1d294d60;
    
    const WORLD: felt252 = 0;

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
        table: felt252,
        keys: Span<felt252>,
        offset: u8,
        value: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    struct StoreDelRecord {
        table: felt252,
        keys: Span<felt252>,
    }

    #[storage]
    struct Storage {
        executor_dispatcher: IExecutorDispatcher,
        middlewares: LegacyMap::<felt252, Middleware>,
        components: LegacyMap::<felt252, ClassHash>,
        systems: LegacyMap::<felt252, ClassHash>,
        nonce: usize,
        owners: LegacyMap::<(felt252, ContractAddress), bool>,
        writers: LegacyMap::<(felt252, felt252), bool>,
        // Tracks the origin executor.
        call_origin: ContractAddress,
        // Tracks the calling systems name for auth purposes.
        call_stack_len: felt252,
        call_stack: LegacyMap::<felt252, felt252>,
    }

    #[constructor]
    fn constructor(ref self: ContractState, executor: ContractAddress) {
        self.executor_dispatcher.write(IExecutorDispatcher { contract_address: executor });
        self
            .owners
            .write((WORLD, starknet::get_tx_info().unbox().account_contract_address), bool::True(()));

        EventEmitter::emit(
            ref self,
            WorldSpawned {
                address: get_contract_address(),
                caller: get_tx_info().unbox().account_contract_address
            }
        );
    }

    #[external(v0)]
    impl World of IWorld<ContractState> {
        /// Checks if the provided account is an owner of the target.
        ///
        /// # Arguments
        ///
        /// * `address` - The contract address.
        /// * `target` - The target.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the address is an owner of the target, false otherwise.
        fn is_owner(self: @ContractState, address: ContractAddress, target: felt252) -> bool {
            self.owners.read((target, address))
        }

        /// Grants ownership of the target to the address.
        /// Can only be called by an existing owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `address` - The contract address.
        /// * `target` - The target.
        fn grant_owner(ref self: ContractState, address: ContractAddress, target: felt252) {
            let caller = get_caller_address();
            assert(
                self.is_owner(caller, target) || self.is_owner(caller, WORLD),
                'not owner'
            );
            self.owners.write((target, address), bool::True(()));
        }

        /// Revokes owner permission to the system for the component.
        /// Can only be called by an existing owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `address` - The contract address.
        /// * `target` - The target.
        fn revoke_owner(ref self: ContractState, address: ContractAddress, target: felt252) {
            let caller = get_caller_address();
            assert(
                self.is_owner(caller, target)
                    || self.is_owner(caller, WORLD),
                'not owner'
            );
            self.owners.write((target, address), bool::False(()));
        }

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
        fn is_writer(self: @ContractState, component: felt252, system: felt252) -> bool {
            self.writers.read((component, system))
        }

        /// Grants writer permission to the system for the component.
        /// Can only be called by an existing component owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `component` - The name of the component.
        /// * `system` - The name of the system.
        fn grant_writer(ref self: ContractState, component: felt252, system: felt252) {
            let caller = get_caller_address();

            assert(
                self.is_owner(caller, component)
                    || self.is_owner(caller, WORLD),
                'not owner or writer'
            );
            self.writers.write((component, system), bool::True(()));
        }

        /// Revokes writer permission to the system for the component.
        /// Can only be called by an existing component writer, owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `component` - The name of the component.
        /// * `system` - The name of the system.
        fn revoke_writer(ref self: ContractState, component: felt252, system: felt252) {
            let caller = get_caller_address();

            assert(
                self.is_writer(component, self.caller_system())
                    || self.is_owner(caller, component)
                    || self.is_owner(caller, WORLD),
                'not owner or writer'
            );
            self.writers.write((component, system), bool::False(()));
        }

        /// Registers a component in the world. If the component is already registered,
        /// the implementation will be updated.
        ///
        /// # Arguments
        ///
        /// * `class_hash` - The class hash of the component to be registered.
        fn register_component(ref self: ContractState, class_hash: ClassHash) {
            let caller = get_caller_address();
            let calldata = ArrayTrait::new();
            let name = *self
                .executor_dispatcher
                .read()
                .call(class_hash, NAME_ENTRYPOINT, calldata.span())[0];

            // If component is already registered, validate permission to update.
            if self.components.read(name).is_non_zero() {
                assert(
                    self.is_owner(caller, name), 'only owner can update'
                );
            } else {
                self.owners.write((name, caller), bool::True(()));
            };

            self.components.write(name, class_hash);
            EventEmitter::emit(ref self, ComponentRegistered { name, class_hash });
        }

        /// Gets the class hash of a registered component.
        ///
        /// # Arguments
        ///
        /// * `name` - The name of the component.
        ///
        /// # Returns
        ///
        /// * `ClassHash` - The class hash of the component.
        fn component(self: @ContractState, name: felt252) -> ClassHash {
            self.components.read(name)
        }

        /// Registers a system in the world. If the system is already registered,
        /// the implementation will be updated.
        ///
        /// # Arguments
        ///
        /// * `class_hash` - The class hash of the system to be registered.
        fn register_system(ref self: ContractState, class_hash: ClassHash) {
            let caller = get_caller_address();
            let calldata = ArrayTrait::new();
            let name = *self
                .executor_dispatcher
                .read()
                .call(class_hash, NAME_ENTRYPOINT, calldata.span())[0];

            // If system is already registered, validate permission to update.
            if self.systems.read(name).is_non_zero() {
                assert(
                    self.is_owner(caller, name), 'only owner can update'
                );
            } else {
                self.owners.write((name, caller), bool::True(()));
            };

            self.systems.write(name, class_hash);
            EventEmitter::emit(ref self, SystemRegistered { name, class_hash });
        }

        /// Gets the class hash of a registered system.
        ///
        /// # Arguments
        ///
        /// * `name` - The name of the system.
        ///
        /// # Returns
        ///
        /// * `ClassHash` - The class hash of the system.
        fn system(self: @ContractState, name: felt252) -> ClassHash {
            self.systems.read(name)
        }

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
        fn execute(
            ref self: ContractState, system: felt252, mut calldata: Array<felt252>
        ) -> Span<felt252> {
            let stack_len = self.call_stack_len.read();
            self.call_stack.write(stack_len, system);
            self.call_stack_len.write(stack_len + 1);

            // Get the class hash of the system to be executed
            let system_class_hash = self.systems.read(system);

            // If this is the initial call, set the origin to the caller
            let mut call_origin = self.call_origin.read();
            if stack_len.is_zero() {
                call_origin = get_caller_address();
                self.call_origin.write(call_origin);
            }

            let ctx = Context {
                world: IWorldDispatcher {
                    contract_address: get_contract_address()
                }, origin: self.call_origin.read(), system, system_class_hash,
            };

            // Add context to calldata
            ctx.serialize(ref calldata);

            // Call the system via executor
            let res = self
                .executor_dispatcher
                .read()
                .execute(ctx.system_class_hash, calldata.span());

            // Reset the current call stack frame
            self.call_stack.write(stack_len, 0);
            // Decrement the call stack pointer
            self.call_stack_len.write(stack_len);

            // If this is the initial call, reset the origin on exit
            if stack_len.is_zero() {
                self.call_origin.write(starknet::contract_address_const::<0x0>());
            }

            res
        }

        /// Issues an autoincremented id to the caller.
        ///
        /// # Returns
        ///
        /// * `usize` - The autoincremented id.
        fn uuid(ref self: ContractState) -> usize {
            let current = self.nonce.read();
            self.nonce.write(current + 1);
            current
        }

        /// Emits a custom event.
        ///
        /// # Arguments
        ///
        /// * `keys` - The keys of the event.
        /// * `values` - The data to be logged by the event.
        fn emit(self: @ContractState, mut keys: Array<felt252>, values: Span<felt252>) {
            self.caller_system().serialize(ref keys);
            emit_event_syscall(keys.span(), values).unwrap_syscall();
        }

        /// Sets the component value for an entity.
        ///
        /// # Arguments
        ///
        /// * `component` - The name of the component to be set.
        /// * `key` - The key to be used to find the entity.
        /// * `offset` - The offset of the component in the entity.
        /// * `value` - The value to be set.
        fn set_entity(
            ref self: ContractState,
            component: felt252,
            keys: Span<felt252>,
            offset: u8,
            value: Span<felt252>
        ) {
            assert_can_write(@self, component);

            let key = poseidon::poseidon_hash_span(keys);
            let component_class_hash = self.components.read(component);
            database::set(component_class_hash, component, key, offset, value);

            EventEmitter::emit(ref self, StoreSetRecord { table: component, keys, offset, value });
        }

        /// Deletes a component from an entity.
        ///
        /// # Arguments
        ///
        /// * `component` - The name of the component to be deleted.
        /// * `query` - The query to be used to find the entity.
        fn delete_entity(ref self: ContractState, component: felt252, keys: Span<felt252>) {
            assert_can_write(@self, component);

            let key = poseidon::poseidon_hash_span(keys);
            let component_class_hash = self.components.read(component);
            database::del(component_class_hash, component, key);

            EventEmitter::emit(ref self, StoreDelRecord { table: component, keys });
        }

        /// Gets the component value for an entity. Returns a zero initialized
        /// component value if the entity has not been set.
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
        /// * `Span<felt252>` - The value of the component, zero initialized if not set.
        fn entity(
            self: @ContractState, component: felt252, keys: Span<felt252>, offset: u8, length: usize
        ) -> Span<felt252> {
            let class_hash = self.components.read(component);
            let key = poseidon::poseidon_hash_span(keys);
            database::get(class_hash, component, key, offset, length)
        }

        /// Returns entity IDs and entities that contain the component state.
        ///
        /// # Arguments
        ///
        /// * `component` - The name of the component to be retrieved.
        /// * `index` - The index to be retrieved.
        ///
        /// # Returns
        ///
        /// * `Span<felt252>` - The entity IDs.
        /// * `Span<Span<felt252>>` - The entities.
        fn entities(
            self: @ContractState, component: felt252, index: felt252, length: usize
        ) -> (Span<felt252>, Span<Span<felt252>>) {
            let class_hash = self.components.read(component);
            database::all(class_hash, component.into(), index, length)
        }

        /// Sets the executor contract address.
        ///
        /// # Arguments
        ///
        /// * `contract_address` - The contract address of the executor.
        fn set_executor(ref self: ContractState, contract_address: ContractAddress) {
            // Only owner can set executor
            assert(self.is_owner(get_caller_address(), WORLD), 'only owner can set executor');
            self
                .executor_dispatcher
                .write(IExecutorDispatcher { contract_address: contract_address });
        }

        /// Gets the executor contract address.
        ///
        /// # Returns
        ///
        /// * `ContractAddress` - The address of the executor contract.
        fn executor(self: @ContractState) -> ContractAddress {
            self.executor_dispatcher.read().contract_address
        }

        /// Gets the origin caller.
        ///
        /// # Returns
        ///
        /// * `felt252` - The origin caller.
        fn origin(self: @ContractState) -> ContractAddress {
            self.call_origin.read()
        }

        /// Gets the caller system's name.
        ///
        /// # Returns
        ///
        /// * `felt252` - The caller system's name.
        fn caller_system(self: @ContractState) -> felt252 {
            self.call_stack.read(self.call_stack_len.read() - 1)
        }
    }

    /// Asserts that the current caller can write to the component.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component being written to.
    fn assert_can_write(self: @ContractState, component: felt252) {
        assert(
            get_caller_address() == self.executor_dispatcher.read().contract_address,
            'must be called thru executor'
        );

        assert(
            IWorld::is_writer(self, component, self.caller_system())
                || IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, component)
                || IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, WORLD),
            'not writer'
        );
    }

    fn register_middleware(&self, middleware: Middleware) {
        let middlewares = self.middleware.read();
        self.middleware.write(middlewares.append(middleware));
    }

    fn unregister_middleware(&self, middleware: Middleware) {
        let middlewares = self.middleware.read();
        let index = middlewares.len();
        loop {
            if index == 0 {
                break;
            }
            index -= 1;
            let value = middlewares.pop_front();
            if value == middleware {
                break;
            }
            middlewares.push_back(value);
        } 
        
        self.middleware.write(middleware);
    }

    fn get_middleware(&self) -> Array<Middleware> {
        self.middleware.read();
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
