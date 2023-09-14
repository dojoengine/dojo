use starknet::{ContractAddress, ClassHash, StorageBaseAddress, SyscallResult};
use traits::{Into, TryInto};
use option::OptionTrait;

#[derive(Copy, Drop, Serde)]
struct Context {
    world: IWorldDispatcher, // Dispatcher to the world contract
    origin: ContractAddress, // Address of the origin
    system: felt252, // Name of the calling system
}


#[starknet::interface]
trait IWorld<T> {
    fn component(self: @T, name: felt252) -> ClassHash;
    fn register_component(ref self: T, class_hash: ClassHash);
    fn system(self: @T, name: felt252) -> ContractAddress;
    fn register_system(ref self: T, class_hash: ClassHash);
    fn register_system_contract(ref self: T, name: felt252, contract_address: ContractAddress);
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
    fn delete_entity(ref self: T, component: felt252, keys: Span<felt252>);

    fn is_owner(self: @T, address: ContractAddress, target: felt252) -> bool;
    fn grant_owner(ref self: T, address: ContractAddress, target: felt252);
    fn revoke_owner(ref self: T, address: ContractAddress, target: felt252);

    fn is_writer(self: @T, component: felt252, system: felt252) -> bool;
    fn grant_writer(ref self: T, component: felt252, system: felt252);
    fn revoke_writer(ref self: T, component: felt252, system: felt252);
}

#[starknet::contract]
mod world {
    const EXECUTE_ENTRYPOINT: felt252 =
        0x0240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44;
    use array::{ArrayTrait, SpanTrait};
    use traits::Into;
    use option::OptionTrait;
    use box::BoxTrait;
    use serde::Serde;
    use starknet::{
        get_caller_address, get_contract_address, get_tx_info,
        contract_address::ContractAddressIntoFelt252, ClassHash, Zeroable, ContractAddress,
        syscalls::emit_event_syscall, SyscallResultTrait, SyscallResultTraitImpl
    };

    use dojo::database;
    use dojo::component::{INamedLibraryDispatcher, INamedDispatcherTrait,};
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
        contract_address: ContractAddress
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
        components: LegacyMap::<felt252, ClassHash>,
        systems: LegacyMap::<felt252, ContractAddress>,
        system_names: LegacyMap::<ContractAddress, felt252>,
        nonce: usize,
        owners: LegacyMap::<(felt252, ContractAddress), bool>,
        writers: LegacyMap::<(felt252, felt252), bool>,
        // Tracks the calling systems name for auth purposes.
        call_stack_len: felt252,
        call_stack: LegacyMap::<felt252, felt252>,
    }

    #[constructor]
    fn constructor(ref self: ContractState) {
        self
            .owners
            .write(
                (WORLD, starknet::get_tx_info().unbox().account_contract_address), bool::True(())
            );

        EventEmitter::emit(
            ref self,
            WorldSpawned {
                address: get_contract_address(),
                caller: get_tx_info().unbox().account_contract_address
            }
        );
    }

    /// Call Helper,
    /// Call the provided `entrypoint` method on the given `class_hash`.
    ///
    /// # Arguments
    ///
    /// * `class_hash` - Class Hash to call.
    /// * `entrypoint` - Entrypoint to call.
    /// * `calldata` - The calldata to pass.
    ///
    /// # Returns
    ///
    /// The return value of the call.
    fn _class_call(
        class_hash: ClassHash, entrypoint: felt252, calldata: Span<felt252>
    ) -> Span<felt252> {
        starknet::syscalls::library_call_syscall(class_hash, entrypoint, calldata).unwrap_syscall()
    }

    /// Gets system name from get caller address
    ///
    /// # Returns
    ///
    /// * `felt252` - The caller system's name.
    fn _system_name_from_caller(self: @ContractState) -> felt252 {
        self.system_names.read(get_caller_address())
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
            assert(self.is_owner(caller, target) || self.is_owner(caller, WORLD), 'not owner');
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
            assert(self.is_owner(caller, target) || self.is_owner(caller, WORLD), 'not owner');
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
                self.is_owner(caller, component) || self.is_owner(caller, WORLD),
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
                self.is_writer(component, _system_name_from_caller(@self))
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
            let name = *_class_call(class_hash, NAME_ENTRYPOINT, calldata.span())[0];

            // If component is already registered, validate permission to update.
            if self.components.read(name).is_non_zero() {
                assert(self.is_owner(caller, name), 'only owner can update');
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

        fn register_system(ref self: ContractState, class_hash: ClassHash) {
            let caller = get_caller_address();
            let name = *_class_call(class_hash, NAME_ENTRYPOINT, array![].span())[0];

            let (system_contract, _) = starknet::deploy_syscall(
                class_hash, 0, array![].span(), false
            )
                .unwrap();

            World::register_system_contract(ref self, name, system_contract)
        }

        /// Registers a system in the world. If the system is already registered,
        /// the implementation will be updated.
        ///
        /// # Arguments
        ///
        /// * `class_hash` - The class hash of the system to be registered.
        fn register_system_contract(
            ref self: ContractState, name: felt252, contract_address: ContractAddress
        ) {
            let caller = get_caller_address();

            let existing_system_contract = self.systems.read(name);

            // If system is already registered
            if existing_system_contract.is_non_zero() {
                // Validate permission to update.
                assert(self.is_owner(caller, name), 'only owner can update');
                // Clear system name for existing contrcat
                self.system_names.write(existing_system_contract, 0);
            } else {
                self.owners.write((name, caller), bool::True(()));
            };

            self.systems.write(name, contract_address);
            self.system_names.write(contract_address, name);
            EventEmitter::emit(ref self, SystemRegistered { name, contract_address });
        }

        /// Gets the class hash of a registered system.
        ///
        /// # Arguments
        ///
        /// * `name` - The name of the system.
        ///
        /// # Returns
        ///
        /// * `ContractAddress` - The contract address of the system.
        fn system(self: @ContractState, name: felt252) -> ContractAddress {
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
            let system_contract_address = self.systems.read(system);

            let ctx = Context {
                world: IWorldDispatcher { contract_address: get_contract_address() },
                origin: get_caller_address(),
                system,
            };

            ctx.serialize(ref calldata);

            starknet::syscalls::call_contract_syscall(
                system_contract_address, EXECUTE_ENTRYPOINT, calldata.span()
            )
                .unwrap_syscall()
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
            let system = _system_name_from_caller(self);
            system.serialize(ref keys);
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
            let system = _system_name_from_caller(@self);
            assert(system.is_non_zero(), 'must be called thru system');
            assert_can_write(@self, component, system);

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
            let system = _system_name_from_caller(@self);
            assert(system.is_non_zero(), 'must be called thru system');
            assert_can_write(@self, component, system);

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
    }

    /// Asserts that the current caller can write to the component.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component being written to.
    /// * `system` - The name of the system writing.
    fn assert_can_write(self: @ContractState, component: felt252, system: felt252) {
        assert(
            IWorld::is_writer(self, component, system)
                || IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, component)
                || IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, WORLD),
            'not writer'
        );
    }
}

#[system]
mod library_call {
    use starknet::{SyscallResultTrait, SyscallResultTraitImpl};

    fn execute(
        class_hash: starknet::ClassHash, entrypoint: felt252, calladata: Span<felt252>
    ) -> Span<felt252> {
        starknet::syscalls::library_call_syscall(class_hash, entrypoint, calladata).unwrap_syscall()
    }
}
