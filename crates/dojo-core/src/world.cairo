use starknet::{ContractAddress, ClassHash, StorageBaseAddress, SyscallResult};
use traits::{Into, TryInto};
use option::OptionTrait;

#[starknet::interface]
trait IWorld<T> {
    fn model(self: @T, name: felt252) -> ClassHash;
    fn register_model(ref self: T, class_hash: ClassHash);
    fn uuid(ref self: T) -> usize;
    fn emit(self: @T, keys: Array<felt252>, values: Span<felt252>);
    fn entity(
        self: @T,
        model: felt252,
        keys: Span<felt252>,
        offset: u8,
        length: usize,
        layout: Span<u8>
    ) -> Span<felt252>;
    fn set_entity(
        ref self: T,
        model: felt252,
        keys: Span<felt252>,
        keys_layout: Span<u8>,
        offset: u8,
        values: Span<felt252>,
        layout: Span<u8>
    );
    fn entities(
        self: @T, model: felt252, index: felt252, keys: Span<felt252>, length: usize, keys_layout: Span<u8>
    ) -> (Span<felt252>, Span<Span<felt252>>);
    fn set_executor(ref self: T, contract_address: ContractAddress);
    fn executor(self: @T) -> ContractAddress;
    fn delete_entity(ref self: T, model: felt252, keys: Span<felt252>, keys_layout: Span<u8>);
    fn is_owner(self: @T, address: ContractAddress, target: felt252) -> bool;
    fn grant_owner(ref self: T, address: ContractAddress, target: felt252);
    fn revoke_owner(ref self: T, address: ContractAddress, target: felt252);

    fn is_writer(self: @T, model: felt252, system: ContractAddress) -> bool;
    fn grant_writer(ref self: T, model: felt252, system: ContractAddress);
    fn revoke_writer(ref self: T, model: felt252, system: ContractAddress);
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
        syscalls::emit_event_syscall, SyscallResultTrait, SyscallResultTraitImpl
    };

    use dojo::database;
    use dojo::executor::{IExecutorDispatcher, IExecutorDispatcherTrait};
    use dojo::world::{IWorldDispatcher, IWorld};

    const NAME_ENTRYPOINT: felt252 =
        0x0361458367e696363fbcc70777d07ebbd2394e89fd0adcaf147faccd1d294d60;

    const WORLD: felt252 = 0;

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        WorldSpawned: WorldSpawned,
        ModelRegistered: ModelRegistered,
        StoreSetRecord: StoreSetRecord,
        StoreDelRecord: StoreDelRecord
    }

    #[derive(Drop, starknet::Event)]
    struct WorldSpawned {
        address: ContractAddress,
        caller: ContractAddress
    }

    #[derive(Drop, starknet::Event)]
    struct ModelRegistered {
        name: felt252,
        class_hash: ClassHash
    }

    #[derive(Drop, starknet::Event)]
    struct StoreSetRecord {
        table: felt252,
        keys: Span<felt252>,
        offset: u8,
        values: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    struct StoreDelRecord {
        table: felt252,
        keys: Span<felt252>,
    }

    #[storage]
    struct Storage {
        executor_dispatcher: IExecutorDispatcher,
        models: LegacyMap::<felt252, ClassHash>,
        nonce: usize,
        owners: LegacyMap::<(felt252, ContractAddress), bool>,
        writers: LegacyMap::<(felt252, ContractAddress), bool>,
        // Tracks the calling systems name for auth purposes.
        call_stack_len: felt252,
        call_stack: LegacyMap::<felt252, felt252>,
    }

    #[constructor]
    fn constructor(ref self: ContractState, executor: ContractAddress) {
        self.executor_dispatcher.write(IExecutorDispatcher { contract_address: executor });
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
    fn class_call(
        self: @ContractState, class_hash: ClassHash, entrypoint: felt252, calldata: Span<felt252>
    ) -> Span<felt252> {
        self.executor_dispatcher.read().call(class_hash, entrypoint, calldata)
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

        /// Revokes owner permission to the system for the model.
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

        /// Checks if the provided system is a writer of the model.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model.
        /// * `system` - The name of the system.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the system is a writer of the model, false otherwise
        fn is_writer(self: @ContractState, model: felt252, system: ContractAddress) -> bool {
            self.writers.read((model, system))
        }

        /// Grants writer permission to the system for the model.
        /// Can only be called by an existing model owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model.
        /// * `system` - The name of the system.
        fn grant_writer(ref self: ContractState, model: felt252, system: ContractAddress) {
            let caller = get_caller_address();

            assert(
                self.is_owner(caller, model) || self.is_owner(caller, WORLD),
                'not owner or writer'
            );
            self.writers.write((model, system), bool::True(()));
        }

        /// Revokes writer permission to the system for the model.
        /// Can only be called by an existing model writer, owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model.
        /// * `system` - The name of the system.
        fn revoke_writer(ref self: ContractState, model: felt252, system: ContractAddress) {
            let caller = get_caller_address();

            assert(
                self.is_writer(model, caller)
                    || self.is_owner(caller, model)
                    || self.is_owner(caller, WORLD),
                'not owner or writer'
            );
            self.writers.write((model, system), bool::False(()));
        }

        /// Registers a model in the world. If the model is already registered,
        /// the implementation will be updated.
        ///
        /// # Arguments
        ///
        /// * `class_hash` - The class hash of the model to be registered.
        fn register_model(ref self: ContractState, class_hash: ClassHash) {
            let caller = get_caller_address();
            let calldata = ArrayTrait::new();
            let name = *class_call(@self, class_hash, NAME_ENTRYPOINT, calldata.span())[0];

            // If model is already registered, validate permission to update.
            if self.models.read(name).is_non_zero() {
                assert(self.is_owner(caller, name), 'only owner can update');
            } else {
                self.owners.write((name, caller), bool::True(()));
            };

            self.models.write(name, class_hash);
            EventEmitter::emit(ref self, ModelRegistered { name, class_hash });
        }

        /// Gets the class hash of a registered model.
        ///
        /// # Arguments
        ///
        /// * `name` - The name of the model.
        ///
        /// # Returns
        ///
        /// * `ClassHash` - The class hash of the model.
        fn model(self: @ContractState, name: felt252) -> ClassHash {
            self.models.read(name)
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
            let system = get_caller_address();
            system.serialize(ref keys);
            emit_event_syscall(keys.span(), values).unwrap_syscall();
        }

        /// Sets the model value for an entity.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model to be set.
        /// * `key` - The key to be used to find the entity.
        /// * `offset` - The offset of the model in the entity.
        /// * `value` - The value to be set.
        fn set_entity(
            ref self: ContractState,
            model: felt252,
            keys: Span<felt252>,
            keys_layout: Span<u8>,
            offset: u8,
            values: Span<felt252>,
            layout: Span<u8>
        ) {
            assert_can_write(@self, model, get_caller_address());

            let key = poseidon::poseidon_hash_span(keys);
            let model_class_hash = self.models.read(model);
            database::set(model_class_hash, model, key, offset, values, layout);

            EventEmitter::emit(ref self, StoreSetRecord { table: model, keys, offset, values });
        }

        /// Deletes a model from an entity.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model to be deleted.
        /// * `query` - The query to be used to find the entity.
        fn delete_entity(ref self: ContractState, model: felt252, keys: Span<felt252>, keys_layout: Span<u8>) {
            let system = get_caller_address();
            assert(system.is_non_zero(), 'must be called thru system');
            assert_can_write(@self, model, system);

            let key = poseidon::poseidon_hash_span(keys);
            let model_class_hash = self.models.read(model);
            database::del(model_class_hash, model, key, keys_layout);

            EventEmitter::emit(ref self, StoreDelRecord { table: model, keys });
        }

        /// Gets the model value for an entity. Returns a zero initialized
        /// model value if the entity has not been set.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model to be retrieved.
        /// * `query` - The query to be used to find the entity.
        /// * `offset` - The offset of the model values.
        /// * `length` - The length of the model values.
        ///
        /// # Returns
        ///
        /// * `Span<felt252>` - The value of the model, zero initialized if not set.
        fn entity(
            self: @ContractState,
            model: felt252,
            keys: Span<felt252>,
            offset: u8,
            length: usize,
            layout: Span<u8>
        ) -> Span<felt252> {
            let class_hash = self.models.read(model);
            let key = poseidon::poseidon_hash_span(keys);
            database::get(class_hash, model, key, offset, length, layout)
        }

        /// Returns entity IDs and entities that contain the model state.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model to be retrieved.
        /// * `index` - The index to be retrieved.
        /// * `keys` - The query to be used to find the entity.
        /// * `length` - The length of the model values.
        ///
        /// # Returns
        ///
        /// * `Span<felt252>` - The entity IDs.
        /// * `Span<Span<felt252>>` - The entities.
        fn entities(
            self: @ContractState, model: felt252, index: felt252, keys: Span<felt252>, length: usize, keys_layout: Span<u8>
        ) -> (Span<felt252>, Span<Span<felt252>>) {
            let class_hash = self.models.read(model);
            assert(keys.len() <= 1, 'Multiple keys not implemented');
            if (keys.len() == 0) {
                database::all(class_hash, model, index, length, keys_layout)
            } else {
                database::get_by_key(class_hash, model.into(), index, *keys.at(0), length, keys_layout)
            } 
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
    }

    /// Asserts that the current caller can write to the model.
    ///
    /// # Arguments
    ///
    /// * `model` - The name of the model being written to.
    /// * `caller` - The name of the caller writing.
    fn assert_can_write(self: @ContractState, model: felt252, caller: ContractAddress) {
        assert(
            IWorld::is_writer(self, model, caller)
                || IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, model)
                || IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, WORLD),
            'not writer'
        );
    }
}
