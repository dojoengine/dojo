use starknet::{ContractAddress, ClassHash, StorageBaseAddress, SyscallResult};
use traits::{Into, TryInto};
use option::OptionTrait;

#[starknet::interface]
trait IWorld<T> {
    fn metadata_uri(self: @T, resource: felt252) -> Span<felt252>;
    fn set_metadata_uri(ref self: T, resource: felt252, uri: Span<felt252>);
    fn model(self: @T, name: felt252) -> ClassHash;
    fn register_model(ref self: T, class_hash: ClassHash);
    fn deploy_contract(ref self: T, salt: felt252, class_hash: ClassHash) -> ContractAddress;
    fn upgrade_contract(ref self: T, address: ContractAddress, class_hash: ClassHash) -> ClassHash;
    fn uuid(ref self: T) -> usize;
    fn emit(self: @T, keys: Array<felt252>, values: Span<felt252>);
    fn entity(
        self: @T, model: felt252, keys: Span<felt252>, offset: u8, length: usize, layout: Span<u8>
    ) -> Span<felt252>;
    fn set_entity(
        ref self: T,
        model: felt252,
        keys: Span<felt252>,
        offset: u8,
        values: Span<felt252>,
        layout: Span<u8>
    );
    fn entities(
        self: @T,
        model: felt252,
        index: Option<felt252>,
        values: Span<felt252>,
        values_length: usize,
        values_layout: Span<u8>
    ) -> (Span<felt252>, Span<Span<felt252>>);
    fn entity_ids(self: @T, model: felt252) -> Span<felt252>;
    fn set_executor(ref self: T, contract_address: ContractAddress);
    fn executor(self: @T) -> ContractAddress;
    fn base(self: @T) -> ClassHash;
    fn delete_entity(ref self: T, model: felt252, keys: Span<felt252>, layout: Span<u8>);
    fn is_owner(self: @T, address: ContractAddress, resource: felt252) -> bool;
    fn grant_owner(ref self: T, address: ContractAddress, resource: felt252);
    fn revoke_owner(ref self: T, address: ContractAddress, resource: felt252);

    fn is_writer(self: @T, model: felt252, system: ContractAddress) -> bool;
    fn grant_writer(ref self: T, model: felt252, system: ContractAddress);
    fn revoke_writer(ref self: T, model: felt252, system: ContractAddress);
}

#[starknet::interface]
trait IUpgradeableWorld<T> {
    fn upgrade(ref self: T, new_class_hash : ClassHash);
}

#[starknet::interface]
trait IWorldProvider<T> {
    fn world(self: @T) -> IWorldDispatcher;
}


#[starknet::contract]
mod world {
    use core::traits::TryInto;
    use array::{ArrayTrait, SpanTrait};
    use traits::Into;
    use option::OptionTrait;
    use box::BoxTrait;
    use starknet::event::EventEmitter;
    use serde::Serde;
    use core::hash::{HashStateExTrait, HashStateTrait};
    use pedersen::{PedersenTrait, HashStateImpl, PedersenImpl};
    use starknet::{
        get_caller_address, get_contract_address, get_tx_info,
        contract_address::ContractAddressIntoFelt252, ClassHash, Zeroable, ContractAddress,
        syscalls::{deploy_syscall, emit_event_syscall, replace_class_syscall}, SyscallResult, SyscallResultTrait,
        SyscallResultTraitImpl
    };

    use dojo::database;
    use dojo::database::index::WhereCondition;
    use dojo::executor::{IExecutorDispatcher, IExecutorDispatcherTrait};
    use dojo::world::{IWorldDispatcher, IWorld, IUpgradeableWorld};
    
    use dojo::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};

    const NAME_ENTRYPOINT: felt252 =
        0x0361458367e696363fbcc70777d07ebbd2394e89fd0adcaf147faccd1d294d60;

    const WORLD: felt252 = 0;

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        WorldSpawned: WorldSpawned,
        ContractDeployed: ContractDeployed,
        ContractUpgraded: ContractUpgraded,
        WorldUpgraded: WorldUpgraded,
        MetadataUpdate: MetadataUpdate,
        ModelRegistered: ModelRegistered,
        StoreSetRecord: StoreSetRecord,
        StoreDelRecord: StoreDelRecord,
        WriterUpdated: WriterUpdated,
        OwnerUpdated: OwnerUpdated,
        ExecutorUpdated: ExecutorUpdated
    }

    #[derive(Drop, starknet::Event)]
    struct WorldSpawned {
        address: ContractAddress,
        creator: ContractAddress
    }

    #[derive(Drop, starknet::Event)]
    struct WorldUpgraded {
        class_hash: ClassHash,
    }
   
    #[derive(Drop, starknet::Event)]
    struct ContractDeployed {
        salt: felt252,
        class_hash: ClassHash,
        address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    struct ContractUpgraded {
        class_hash: ClassHash,
        address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    struct MetadataUpdate {
        resource: felt252,
        uri: Span<felt252>
    }

    #[derive(Drop, starknet::Event)]
    struct ModelRegistered {
        name: felt252,
        class_hash: ClassHash,
        prev_class_hash: ClassHash
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

    #[derive(Drop, starknet::Event)]
    struct WriterUpdated {
        model: felt252,
        system: ContractAddress,
        value: bool
    }

    #[derive(Drop, starknet::Event)]
    struct OwnerUpdated {
        address: ContractAddress,
        resource: felt252,
        value: bool,
    }

    #[derive(Drop, starknet::Event)]
    struct ExecutorUpdated {
        address: ContractAddress,
        prev_address: ContractAddress,
    }


    #[storage]
    struct Storage {
        executor_dispatcher: IExecutorDispatcher,
        contract_base: ClassHash,
        nonce: usize,
        metadata_uri: LegacyMap::<felt252, felt252>,
        models: LegacyMap::<felt252, ClassHash>,
        owners: LegacyMap::<(felt252, ContractAddress), bool>,
        writers: LegacyMap::<(felt252, ContractAddress), bool>,
    }

    #[constructor]
    fn constructor(ref self: ContractState, executor: ContractAddress, contract_base: ClassHash) {
        let creator = starknet::get_tx_info().unbox().account_contract_address;
        self.executor_dispatcher.write(IExecutorDispatcher { contract_address: executor });
        self.contract_base.write(contract_base);
        self.owners.write((WORLD, creator), true);

        EventEmitter::emit(ref self, WorldSpawned { address: get_contract_address(), creator });
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
        /// Returns the metadata URI of the world.
        ///
        /// # Returns
        ///
        /// * `Span<felt252>` - The metadata URI of the world.
        fn metadata_uri(self: @ContractState, resource: felt252) -> Span<felt252> {
            let mut uri = array![];

            // We add one here since we start i at 1;
            let len = self.metadata_uri.read(resource) + 1;

            let mut i = resource + 1;
            loop {
                if len == i {
                    break;
                }

                uri.append(self.metadata_uri.read(i));
                i += 1;
            };

            uri.span()
        }

        /// Sets the metadata URI of the world.
        ///
        /// # Arguments
        ///
        /// * `uri` - The new metadata URI to be set.
        fn set_metadata_uri(ref self: ContractState, resource: felt252, mut uri: Span<felt252>) {
            assert(self.is_owner(get_caller_address(), resource), 'not owner');

            let len = uri.len();

            // Max len to avoid overflowing into other resources
            assert(len < 255, 'metadata too long');

            self.metadata_uri.write(resource, len.into());

            // Emit event before uri is consumed.
            EventEmitter::emit(ref self, MetadataUpdate { resource, uri });

            let mut i = resource + 1;
            loop {
                match uri.pop_front() {
                    Option::Some(item) => {
                        self.metadata_uri.write(i, *item);
                        i += 1;
                    },
                    Option::None(_) => { break; }
                };
            };
        }

        /// Checks if the provided account is an owner of the resource.
        ///
        /// # Arguments
        ///
        /// * `address` - The contract address.
        /// * `resource` - The resource.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the address is an owner of the resource, false otherwise.
        fn is_owner(self: @ContractState, address: ContractAddress, resource: felt252) -> bool {
            self.owners.read((resource, address))
        }

        /// Grants ownership of the resource to the address.
        /// Can only be called by an existing owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `address` - The contract address.
        /// * `resource` - The resource.
        fn grant_owner(ref self: ContractState, address: ContractAddress, resource: felt252) {
            let caller = get_caller_address();
            assert(self.is_owner(caller, resource) || self.is_owner(caller, WORLD), 'not owner');
            self.owners.write((resource, address), true);

            EventEmitter::emit(ref self, OwnerUpdated { address, resource, value: true });
        }

        /// Revokes owner permission to the system for the model.
        /// Can only be called by an existing owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `address` - The contract address.
        /// * `resource` - The resource.
        fn revoke_owner(ref self: ContractState, address: ContractAddress, resource: felt252) {
            let caller = get_caller_address();
            assert(self.is_owner(caller, resource) || self.is_owner(caller, WORLD), 'not owner');
            self.owners.write((resource, address), bool::False(()));

            EventEmitter::emit(ref self, OwnerUpdated { address, resource, value: false });
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
                self.is_owner(caller, model) || self.is_owner(caller, WORLD), 'not owner or writer'
            );
            self.writers.write((model, system), true);

            EventEmitter::emit(ref self, WriterUpdated { model, system, value: true });
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
            self.writers.write((model, system), false);

            EventEmitter::emit(ref self, WriterUpdated { model, system, value: false });
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
            let mut prev_class_hash = starknet::class_hash::ClassHashZeroable::zero();

            // If model is already registered, validate permission to update.
            let current_class_hash = self.models.read(name);
            if current_class_hash.is_non_zero() {
                assert(self.is_owner(caller, name), 'only owner can update');
                prev_class_hash = current_class_hash;
            } else {
                self.owners.write((name, caller), true);
            };

            self.models.write(name, class_hash);
            EventEmitter::emit(ref self, ModelRegistered { name, class_hash, prev_class_hash });
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

        /// Deploys a contract associated with the world.
        ///
        /// # Arguments
        ///
        /// * `name` - The name of the contract.
        /// * `class_hash` - The class_hash of the contract.
        ///
        /// # Returns
        ///
        /// * `ContractAddress` - The address of the newly deployed contract.
        fn deploy_contract(
            ref self: ContractState, salt: felt252, class_hash: ClassHash
        ) -> ContractAddress {
            let (contract_address, _) = deploy_syscall(
                self.contract_base.read(), salt, array![].span(), false
            )
                .unwrap_syscall();
            let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address };
            upgradeable_dispatcher.upgrade(class_hash);

            self.owners.write((contract_address.into(), get_caller_address()), true);

            EventEmitter::emit(
                ref self, ContractDeployed { salt, class_hash, address: contract_address }
            );

            contract_address
        }

        /// Upgrade an already deployed contract associated with the world.
        ///
        /// # Arguments
        ///
        /// * `name` - The name of the contract.
        /// * `class_hash` - The class_hash of the contract.
        ///
        /// # Returns
        ///
        /// * `ClassHash` - The new class hash of the contract.
        fn upgrade_contract(
            ref self: ContractState, address: ContractAddress, class_hash: ClassHash
        ) -> ClassHash {
            // Only owner can upgrade contract
            assert_can_write(@self, address.into(), get_caller_address());
            IUpgradeableDispatcher { contract_address: address }.upgrade(class_hash);
            EventEmitter::emit(ref self, ContractUpgraded { class_hash, address });
            class_hash
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
            offset: u8,
            values: Span<felt252>,
            layout: Span<u8>
        ) {
            assert_can_write(@self, model, get_caller_address());

            let key = poseidon::poseidon_hash_span(keys);
            database::set(model, key, offset, values, layout);

            EventEmitter::emit(ref self, StoreSetRecord { table: model, keys, offset, values });
        }

        /// Deletes a model from an entity.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model to be deleted.
        /// * `query` - The query to be used to find the entity.
        fn delete_entity(
            ref self: ContractState, model: felt252, keys: Span<felt252>, layout: Span<u8>
        ) {
            assert_can_write(@self, model, get_caller_address());

            let model_class_hash = self.models.read(model);

            let mut empty_values = ArrayTrait::new();
            let mut i = 0;

            loop {
                if (i == layout.len()) {
                    break;
                }
                empty_values.append(0);
                i += 1;
            };

            let key = poseidon::poseidon_hash_span(keys);
            database::set(model, key, 0, empty_values.span(), layout);
            // this deletes the index
            database::del(model, key);

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
            let key = poseidon::poseidon_hash_span(keys);
            database::get(model, key, offset, length, layout)
        }

        /// Returns entity IDs and entities that contain the model state.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model to be retrieved.
        /// * `index` - The index to be retrieved.
        /// * `values` - The query to be used to find the entity.
        /// * `length` - The length of the model values.
        ///
        /// # Returns
        ///
        /// * `Span<felt252>` - The entity IDs.
        /// * `Span<Span<felt252>>` - The entities.
        fn entities(
            self: @ContractState,
            model: felt252,
            index: Option<felt252>,
            values: Span<felt252>,
            values_length: usize,
            values_layout: Span<u8>
        ) -> (Span<felt252>, Span<Span<felt252>>) {
            assert(values.len() == 0, 'Queries by values not impl');
            database::scan(model, Option::None(()), values_length, values_layout)
        }

        /// Returns only the entity IDs that contain the model state.
        /// # Arguments
        /// * `model` - The name of the model to be retrieved.
        /// * `index` - The index to be retrieved.
        /// * `values` - The query to be used to find the entity.
        /// * `length` - The length of the model values.
        ///
        /// # Returns
        /// * `Span<felt252>` - The entity IDs.
        /// * `Span<Span<felt252>>` - The entities.
        fn entity_ids(self: @ContractState, model: felt252) -> Span<felt252> {
            database::scan_ids(model, Option::None(()))
        }

        /// Sets the executor contract address.
        ///
        /// # Arguments
        ///
        /// * `contract_address` - The contract address of the executor.
        fn set_executor(ref self: ContractState, contract_address: ContractAddress) {
            // Only owner can set executor
            assert(self.is_owner(get_caller_address(), WORLD), 'only owner can set executor');
            let prev_address = self.executor_dispatcher.read().contract_address;
            self
                .executor_dispatcher
                .write(IExecutorDispatcher { contract_address: contract_address });

            EventEmitter::emit(
                ref self, ExecutorUpdated { address: contract_address, prev_address }
            );
        }

        /// Gets the executor contract address.
        ///
        /// # Returns
        ///
        /// * `ContractAddress` - The address of the executor contract.
        fn executor(self: @ContractState) -> ContractAddress {
            self.executor_dispatcher.read().contract_address
        }

        /// Gets the base contract class hash.
        ///
        /// # Returns
        ///
        /// * `ClassHash` - The class_hash of the contract_base contract.
        fn base(self: @ContractState) -> ClassHash {
            self.contract_base.read()
        }
    }


    #[external(v0)]
    impl UpgradeableWorld of IUpgradeableWorld<ContractState> {
        /// Upgrade world with new_class_hash
        ///
        /// # Arguments
        ///
        /// * `new_class_hash` - The new world class hash.
        fn upgrade(ref self: ContractState, new_class_hash : ClassHash){
            assert(new_class_hash.is_non_zero(), 'invalid class_hash');
            assert(IWorld::is_owner(@self, get_tx_info().unbox().account_contract_address, WORLD), 'only owner can upgrade');

            // upgrade to new_class_hash
            replace_class_syscall(new_class_hash).unwrap();

            // emit Upgrade Event
            EventEmitter::emit(
                ref self, WorldUpgraded {class_hash: new_class_hash }
            );
        }
    }

    /// Asserts that the current caller can write to the model.
    ///
    /// # Arguments
    ///
    /// * `resource` - The name of the resource being written to.
    /// * `caller` - The name of the caller writing.
    fn assert_can_write(self: @ContractState, resource: felt252, caller: ContractAddress) {
        assert(
            IWorld::is_writer(self, resource, caller)
                || IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, resource)
                || IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, WORLD),
            'not writer'
        );
    }
}
