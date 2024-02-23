use starknet::{ContractAddress, ClassHash, StorageBaseAddress, SyscallResult};
use traits::{Into, TryInto};
use option::OptionTrait;
use dojo::resource_metadata::ResourceMetadata;

#[starknet::interface]
trait IWorld<T> {
    fn metadata(self: @T, resource_id: felt252) -> ResourceMetadata;
    fn set_metadata(ref self: T, metadata: ResourceMetadata);
    fn model(self: @T, name: felt252) -> (ClassHash, ContractAddress);
    fn register_model(ref self: T, class_hash: ClassHash);
    fn deploy_contract(ref self: T, salt: felt252, class_hash: ClassHash) -> ContractAddress;
    fn upgrade_contract(ref self: T, address: ContractAddress, class_hash: ClassHash) -> ClassHash;
    fn uuid(ref self: T) -> usize;
    fn emit(self: @T, keys: Array<felt252>, values: Span<felt252>);
    fn entity(
        self: @T, model: felt252, keys: Span<felt252>, layout: Span<u8>
    ) -> Span<felt252>;
    fn set_entity(
        ref self: T,
        model: felt252,
        keys: Span<felt252>,
        values: Span<felt252>,
        layout: Span<u8>
    );
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
    fn upgrade(ref self: T, new_class_hash: ClassHash);
}

#[starknet::interface]
trait IWorldProvider<T> {
    fn world(self: @T) -> IWorldDispatcher;
}

#[starknet::interface]
trait IDojoResourceProvider<T> {
    fn dojo_resource(self: @T) -> felt252;
}

mod Errors {
    const METADATA_DESER: felt252 = 'metadata deser error';
    const NOT_OWNER: felt252 = 'not owner';
    const NOT_OWNER_WRITER: felt252 = 'not owner or writer';
    const INVALID_MODEL_NAME: felt252 = 'invalid model name';
    const OWNER_ONLY_UPGRADE: felt252 = 'only owner can upgrade';
    const OWNER_ONLY_UPDATE: felt252 = 'only owner can update';
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
        syscalls::{deploy_syscall, emit_event_syscall, replace_class_syscall}, SyscallResult,
        SyscallResultTrait, SyscallResultTraitImpl
    };

    use dojo::database;
    use dojo::database::introspect::Introspect;
    use dojo::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
    use dojo::model::Model;
    use dojo::world::{IWorldDispatcher, IWorld, IUpgradeableWorld};
    use dojo::resource_metadata;
    use dojo::resource_metadata::{ResourceMetadata, RESOURCE_METADATA_MODEL};

    use super::Errors;

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
        prev_class_hash: ClassHash,
        address: ContractAddress,
        prev_address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    struct StoreSetRecord {
        table: felt252,
        keys: Span<felt252>,
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

    #[storage]
    struct Storage {
        contract_base: ClassHash,
        nonce: usize,
        models_count: usize,
        models: LegacyMap::<felt252, (ClassHash, ContractAddress)>,
        deployed_contracts: LegacyMap::<felt252, ClassHash>,
        owners: LegacyMap::<(felt252, ContractAddress), bool>,
        writers: LegacyMap::<(felt252, ContractAddress), bool>,
    }

    #[constructor]
    fn constructor(ref self: ContractState, contract_base: ClassHash) {
        let creator = starknet::get_tx_info().unbox().account_contract_address;
        self.contract_base.write(contract_base);
        self.owners.write((WORLD, creator), true);
        self.models.write(
            RESOURCE_METADATA_MODEL,
            (resource_metadata::initial_class_hash(), resource_metadata::initial_address())
        );

        EventEmitter::emit(ref self, WorldSpawned { address: get_contract_address(), creator });
    }

    #[abi(embed_v0)]
    impl World of IWorld<ContractState> {
        /// Returns the metadata of the resource.
        ///
        /// # Arguments
        ///
        /// `resource_id` - The resource id.
        fn metadata(self: @ContractState, resource_id: felt252) -> ResourceMetadata {
            let mut layout = array![];
            Introspect::<ResourceMetadata>::layout(ref layout);

            let mut data = self
                .entity(RESOURCE_METADATA_MODEL, array![resource_id].span(), layout.span(),);

            let mut model = array![resource_id];
            core::array::serialize_array_helper(data, ref model);

            let mut model_span = model.span();

            Serde::<ResourceMetadata>::deserialize(ref model_span).expect(Errors::METADATA_DESER)
        }

        /// Sets the metadata of the resource.
        ///
        /// # Arguments
        ///
        /// `metadata` - The metadata content for the resource.
        fn set_metadata(ref self: ContractState, metadata: ResourceMetadata) {
            assert_can_write(@self, metadata.resource_id, get_caller_address());

            let model = Model::<ResourceMetadata>::name(@metadata);
            let keys = Model::<ResourceMetadata>::keys(@metadata);
            let values = Model::<ResourceMetadata>::values(@metadata);
            let layout = Model::<ResourceMetadata>::layout(@metadata);

            let key = poseidon::poseidon_hash_span(keys);
            database::set(model, key, values, layout);

            EventEmitter::emit(ref self, MetadataUpdate { resource: metadata.resource_id, uri: metadata.metadata_uri });
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
            assert(self.is_owner(caller, resource) || self.is_owner(caller, WORLD), Errors::NOT_OWNER);
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
            assert(self.is_owner(caller, resource) || self.is_owner(caller, WORLD), Errors::NOT_OWNER);
            self.owners.write((resource, address), false);

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
                self.is_owner(caller, model) || self.is_owner(caller, WORLD), Errors::NOT_OWNER_WRITER
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
                Errors::NOT_OWNER_WRITER
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

            let salt = self.models_count.read();
            let (address, name) = dojo::model::deploy_and_get_name(salt.into(), class_hash).unwrap_syscall();
            self.models_count.write(salt + 1);

            let (mut prev_class_hash, mut prev_address) = (
                starknet::class_hash::ClassHashZeroable::zero(),
                starknet::contract_address::ContractAddressZeroable::zero(),
            );

            // Avoids a model name to conflict with already deployed contract,
            // which can cause ACL issue with current ACL implementation.
            if name.is_zero() || self.deployed_contracts.read(name).is_non_zero() {
                panic_with_felt252(Errors::INVALID_MODEL_NAME);
            }

            // If model is already registered, validate permission to update.
            let (current_class_hash, current_address) = self.models.read(name);
            if current_class_hash.is_non_zero() {
                assert(self.is_owner(caller, name), Errors::OWNER_ONLY_UPDATE);
                prev_class_hash = current_class_hash;
                prev_address = current_address;
            } else {
                self.owners.write((name, caller), true);
            };

            self.models.write(name, (class_hash, address));
            EventEmitter::emit(ref self, ModelRegistered { name, prev_address, address, class_hash, prev_class_hash });
        }

        /// Gets the class hash of a registered model.
        ///
        /// # Arguments
        ///
        /// * `name` - The name of the model.
        ///
        /// # Returns
        ///
        /// * (`ClassHash`, `ContractAddress`) - The class hash and the contract address of the model.
        fn model(self: @ContractState, name: felt252) -> (ClassHash, ContractAddress) {
            self.models.read(name)
        }

        /// Deploys a contract associated with the world.
        ///
        /// # Arguments
        ///
        /// * `salt` - The salt use for contract deployment.
        /// * `class_hash` - The class hash of the contract.
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

            self.deployed_contracts.write(contract_address.into(), class_hash.into());

            EventEmitter::emit(
                ref self, ContractDeployed { salt, class_hash, address: contract_address }
            );

            contract_address
        }

        /// Upgrades an already deployed contract associated with the world.
        ///
        /// # Arguments
        ///
        /// * `address` - The contract address of the contract to upgrade.
        /// * `class_hash` - The class hash of the contract.
        ///
        /// # Returns
        ///
        /// * `ClassHash` - The new class hash of the contract.
        fn upgrade_contract(
            ref self: ContractState, address: ContractAddress, class_hash: ClassHash
        ) -> ClassHash {
            assert(is_account_owner(@self, address.into()), Errors::NOT_OWNER);
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
        /// * `keys` - The key to be used to find the entity.
        /// * `values` - The value to be set.
        /// * `layout` - The memory layout of the entity.
        fn set_entity(
            ref self: ContractState,
            model: felt252,
            keys: Span<felt252>,
            values: Span<felt252>,
            layout: Span<u8>
        ) {
            assert_can_write(@self, model, get_caller_address());

            let key = poseidon::poseidon_hash_span(keys);
            database::set(model, key, values, layout);

            EventEmitter::emit(ref self, StoreSetRecord { table: model, keys, values });
        }

        /// Deletes a model from an entity.
        /// Deleting is setting all the values to 0 in the given layout.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model to be deleted.
        /// * `keys` - The key to be used to find the entity.
        /// * `layout` - The memory layout of the entity.
        fn delete_entity(
            ref self: ContractState, model: felt252, keys: Span<felt252>, layout: Span<u8>
        ) {
            assert_can_write(@self, model, get_caller_address());

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
            database::set(model, key, empty_values.span(), layout);

            EventEmitter::emit(ref self, StoreDelRecord { table: model, keys });
        }

        /// Gets the model value for an entity. Returns a zero initialized
        /// model value if the entity has not been set.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model to be retrieved.
        /// * `keys` - The keys used to find the entity.
        /// * `layout` - The memory layout of the entity.
        ///
        /// # Returns
        ///
        /// * `Span<felt252>` - The serialized value of the model, zero initialized if not set.
        fn entity(
            self: @ContractState,
            model: felt252,
            keys: Span<felt252>,
            layout: Span<u8>
        ) -> Span<felt252> {
            let key = poseidon::poseidon_hash_span(keys);
            database::get(model, key, layout)
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


    #[abi(embed_v0)]
    impl UpgradeableWorld of IUpgradeableWorld<ContractState> {
        /// Upgrades the world with new_class_hash
        ///
        /// # Arguments
        ///
        /// * `new_class_hash` - The new world class hash.
        fn upgrade(ref self: ContractState, new_class_hash: ClassHash) {
            assert(new_class_hash.is_non_zero(), 'invalid class_hash');
            assert(
                IWorld::is_owner(@self, get_tx_info().unbox().account_contract_address, WORLD),
                Errors::OWNER_ONLY_UPGRADE,
            );

            // upgrade to new_class_hash
            replace_class_syscall(new_class_hash).unwrap();

            // emit Upgrade Event
            EventEmitter::emit(ref self, WorldUpgraded { class_hash: new_class_hash });
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
            IWorld::is_writer(self, resource, caller) || is_account_owner(self, resource),
            'not writer'
        );
    }

    /// Verifies if the calling account is owner of the resource or the
    /// owner of the world.
    ///
    /// # Arguments
    ///
    /// * `resource` - The name of the resource being verified.
    ///
    /// # Returns
    ///
    /// * `bool` - True if the calling account is the owner of the resource or the owner of the world,
    ///            false otherwise.
    fn is_account_owner(self: @ContractState, resource: felt252) -> bool {
        IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, resource)
            || IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, WORLD)
    }
}
