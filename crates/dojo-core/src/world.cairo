use starknet::{ContractAddress, ClassHash, StorageBaseAddress, SyscallResult};
use traits::{Into, TryInto};
use option::OptionTrait;
use dojo::resource_metadata::ResourceMetadata;

#[starknet::interface]
trait IWorld<T> {
    fn metadata(self: @T, resource_id: felt252) -> ResourceMetadata;
    fn set_metadata(ref self: T, metadata: ResourceMetadata);
    fn model(self: @T, selector: felt252) -> (ClassHash, ContractAddress);
    fn register_model(ref self: T, class_hash: ClassHash);
    fn deploy_contract(
        ref self: T, salt: felt252, class_hash: ClassHash, init_calldata: Span<felt252>
    ) -> ContractAddress;
    fn upgrade_contract(ref self: T, address: ContractAddress, class_hash: ClassHash) -> ClassHash;
    fn uuid(ref self: T) -> usize;
    fn emit(self: @T, keys: Array<felt252>, values: Span<felt252>);
    fn entity(
        self: @T, model: felt252, keys: Span<felt252>, layout: dojo::database::introspect::Layout
    ) -> Span<felt252>;
    fn set_entity(
        ref self: T,
        model: felt252,
        keys: Span<felt252>,
        values: Span<felt252>,
        layout: dojo::database::introspect::Layout
    );
    fn delete_entity(
        ref self: T, model: felt252, keys: Span<felt252>, layout: dojo::database::introspect::Layout
    );
    fn base(self: @T) -> ClassHash;
    fn is_owner(self: @T, address: ContractAddress, resource: felt252) -> bool;
    fn grant_owner(ref self: T, address: ContractAddress, resource: felt252);
    fn revoke_owner(ref self: T, address: ContractAddress, resource: felt252);

    fn is_writer(self: @T, model: felt252, contract: ContractAddress) -> bool;
    fn grant_writer(ref self: T, model: felt252, contract: ContractAddress);
    fn revoke_writer(ref self: T, model: felt252, contract: ContractAddress);
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
    use dojo::config::interface::IConfig;
    use core::to_byte_array::FormatAsByteArray;
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
        contract_address_const, get_caller_address, get_contract_address, get_tx_info,
        contract_address::ContractAddressIntoFelt252, ClassHash, Zeroable, ContractAddress,
        syscalls::{deploy_syscall, emit_event_syscall, replace_class_syscall}, SyscallResult,
        SyscallResultTrait, SyscallResultTraitImpl
    };

    use dojo::database;
    use dojo::database::introspect::{Introspect, Layout, FieldLayout};
    use dojo::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
    use dojo::config::component::Config;
    use dojo::model::Model;
    use dojo::interfaces::{
        IUpgradeableState, IFactRegistryDispatcher, IFactRegistryDispatcherImpl, StorageUpdate,
        ProgramOutput
    };
    use dojo::world::{IWorldDispatcher, IWorld, IUpgradeableWorld};
    use dojo::resource_metadata;
    use dojo::resource_metadata::{ResourceMetadata, RESOURCE_METADATA_SELECTOR};

    use super::Errors;

    const WORLD: felt252 = 0;

    // the minimum internal size of an empty ByteArray
    const MIN_BYTE_ARRAY_SIZE: u32 = 3;

    const DOJO_INIT_SELECTOR: felt252 = selector!("dojo_init");

    component!(path: Config, storage: config, event: ConfigEvent);

    #[abi(embed_v0)]
    impl ConfigImpl = Config::ConfigImpl<ContractState>;

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
        ConfigEvent: Config::Event,
        StateUpdated: StateUpdated
    }

    #[derive(Drop, starknet::Event)]
    struct StateUpdated {
        da_hash: felt252,
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
        uri: ByteArray
    }

    #[derive(Drop, starknet::Event)]
    struct ModelRegistered {
        name: ByteArray,
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
        contract: ContractAddress,
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
        #[substorage(v0)]
        config: Config::Storage,
        initialized_contract: LegacyMap::<felt252, bool>,
    }

    #[constructor]
    fn constructor(ref self: ContractState, contract_base: ClassHash) {
        let creator = starknet::get_tx_info().unbox().account_contract_address;
        self.contract_base.write(contract_base);
        self.owners.write((WORLD, creator), true);
        self
            .models
            .write(
                RESOURCE_METADATA_SELECTOR,
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
            let mut data = self
                ._read_model_data(
                    RESOURCE_METADATA_SELECTOR,
                    array![resource_id].span(),
                    Model::<ResourceMetadata>::layout()
                );

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

            let model = Model::<ResourceMetadata>::selector();
            let keys = Model::<ResourceMetadata>::keys(@metadata);
            let values = Model::<ResourceMetadata>::values(@metadata);
            let layout = Model::<ResourceMetadata>::layout();

            self._write_model_data(model, keys, values, layout);

            EventEmitter::emit(
                ref self,
                MetadataUpdate { resource: metadata.resource_id, uri: metadata.metadata_uri }
            );
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
            assert(
                self.is_owner(caller, resource) || self.is_owner(caller, WORLD), Errors::NOT_OWNER
            );
            self.owners.write((resource, address), true);

            EventEmitter::emit(ref self, OwnerUpdated { address, resource, value: true });
        }

        /// Revokes owner permission to the contract for the model.
        /// Can only be called by an existing owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `address` - The contract address.
        /// * `resource` - The resource.
        fn revoke_owner(ref self: ContractState, address: ContractAddress, resource: felt252) {
            let caller = get_caller_address();
            assert(
                self.is_owner(caller, resource) || self.is_owner(caller, WORLD), Errors::NOT_OWNER
            );
            self.owners.write((resource, address), false);

            EventEmitter::emit(ref self, OwnerUpdated { address, resource, value: false });
        }

        /// Checks if the provided contract is a writer of the model.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model.
        /// * `contract` - The name of the contract.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the contract is a writer of the model, false otherwise
        fn is_writer(self: @ContractState, model: felt252, contract: ContractAddress) -> bool {
            self.writers.read((model, contract))
        }

        /// Grants writer permission to the contract for the model.
        /// Can only be called by an existing model owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model.
        /// * `contract` - The name of the contract.
        fn grant_writer(ref self: ContractState, model: felt252, contract: ContractAddress) {
            let caller = get_caller_address();

            assert(
                self.is_owner(caller, model) || self.is_owner(caller, WORLD),
                Errors::NOT_OWNER_WRITER
            );
            self.writers.write((model, contract), true);

            EventEmitter::emit(ref self, WriterUpdated { model, contract, value: true });
        }

        /// Revokes writer permission to the contract for the model.
        /// Can only be called by an existing model writer, owner or the world admin.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model.
        /// * `contract` - The name of the contract.
        fn revoke_writer(ref self: ContractState, model: felt252, contract: ContractAddress) {
            let caller = get_caller_address();

            assert(
                self.is_writer(model, caller)
                    || self.is_owner(caller, model)
                    || self.is_owner(caller, WORLD),
                Errors::NOT_OWNER_WRITER
            );
            self.writers.write((model, contract), false);

            EventEmitter::emit(ref self, WriterUpdated { model, contract, value: false });
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
            let (address, name, selector) = dojo::model::deploy_and_get_metadata(
                salt.into(), class_hash
            )
                .unwrap_syscall();
            self.models_count.write(salt + 1);

            let (mut prev_class_hash, mut prev_address) = (
                starknet::class_hash::ClassHashZeroable::zero(),
                starknet::contract_address::ContractAddressZeroable::zero(),
            );

            // Avoids a model name to conflict with already deployed contract,
            // which can cause ACL issue with current ACL implementation.
            if selector.is_zero() || self.deployed_contracts.read(selector).is_non_zero() {
                panic_with_felt252(Errors::INVALID_MODEL_NAME);
            }

            // If model is already registered, validate permission to update.
            let (current_class_hash, current_address) = self.models.read(selector);
            if current_class_hash.is_non_zero() {
                assert(self.is_owner(caller, selector), Errors::OWNER_ONLY_UPDATE);
                prev_class_hash = current_class_hash;
                prev_address = current_address;
            } else {
                self.owners.write((selector, caller), true);
            };

            self.models.write(selector, (class_hash, address));
            EventEmitter::emit(
                ref self,
                ModelRegistered { name, prev_address, address, class_hash, prev_class_hash }
            );
        }

        /// Gets the class hash of a registered model.
        ///
        /// # Arguments
        ///
        /// * `selector` - The keccak(name) of the model.
        ///
        /// # Returns
        ///
        /// * (`ClassHash`, `ContractAddress`) - The class hash and the contract address of the model.
        fn model(self: @ContractState, selector: felt252) -> (ClassHash, ContractAddress) {
            self.models.read(selector)
        }

        /// Deploys a contract associated with the world.
        ///
        /// # Arguments
        ///
        /// * `salt` - The salt use for contract deployment.
        /// * `class_hash` - The class hash of the contract.
        /// * `init_calldata` - Calldata used to initialize the contract.
        ///
        /// # Returns
        ///
        /// * `ContractAddress` - The address of the newly deployed contract.
        fn deploy_contract(
            ref self: ContractState,
            salt: felt252,
            class_hash: ClassHash,
            init_calldata: Span<felt252>,
        ) -> ContractAddress {
            let (contract_address, _) = deploy_syscall(
                self.contract_base.read(), salt, array![].span(), false
            )
                .unwrap_syscall();
            let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address };
            upgradeable_dispatcher.upgrade(class_hash);

            if self.initialized_contract.read(contract_address.into()) {
                panic!("Contract has been already initialized");
            } else {
                starknet::call_contract_syscall(contract_address, DOJO_INIT_SELECTOR, init_calldata)
                    .unwrap_syscall();
                self.initialized_contract.write(contract_address.into(), true);
            }

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
        /// * `model` - The selector of the model to be set.
        /// * `keys` - The key to be used to find the entity.
        /// * `value_names` - The name of model fields which are not a key.
        /// * `values` - The value to be set.
        /// * `layout` - The memory layout of the entity.
        fn set_entity(
            ref self: ContractState,
            model: felt252,
            keys: Span<felt252>,
            values: Span<felt252>,
            layout: dojo::database::introspect::Layout
        ) {
            assert_can_write(@self, model, get_caller_address());

            self._write_model_data(model, keys, values, layout);
            EventEmitter::emit(ref self, StoreSetRecord { table: model, keys, values });
        }

        /// Deletes a model from an entity.
        /// Deleting is setting all the values to 0 in the given layout.
        ///
        /// # Arguments
        ///
        /// * `model` - The selector of the model to be deleted.
        /// * `keys` - The key to be used to find the entity.
        /// * `value_names` - The name of model fields which are not a key.
        /// * `layout` - The memory layout of the entity.
        fn delete_entity(
            ref self: ContractState,
            model: felt252,
            keys: Span<felt252>,
            layout: dojo::database::introspect::Layout
        ) {
            assert_can_write(@self, model, get_caller_address());

            self._delete_model_data(model, keys, layout);
            EventEmitter::emit(ref self, StoreDelRecord { table: model, keys });
        }

        /// Gets the model value for an entity. Returns a zero initialized
        /// model value if the entity has not been set.
        ///
        /// # Arguments
        ///
        /// * `model` - The selector of the model to be retrieved.
        /// * `keys` - The keys used to find the entity.
        /// * `value_names` - The name of model fields which are not a key.
        /// * `layout` - The memory layout of the entity.
        ///
        /// # Returns
        ///
        /// * `Span<felt252>` - The serialized value of the model, zero initialized if not set.
        fn entity(
            self: @ContractState,
            model: felt252,
            keys: Span<felt252>,
            layout: dojo::database::introspect::Layout
        ) -> Span<felt252> {
            self._read_model_data(model, keys, layout)
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

    #[abi(embed_v0)]
    impl UpgradeableState of IUpgradeableState<ContractState> {
        fn upgrade_state(
            ref self: ContractState, new_state: Span<StorageUpdate>, program_output: ProgramOutput
        ) {
            let mut da_hasher = PedersenImpl::new(0);
            let mut i = 0;
            loop {
                if i == new_state.len() {
                    break;
                }
                da_hasher = da_hasher.update(*new_state.at(i).key);
                da_hasher = da_hasher.update(*new_state.at(i).value);
                i += 1;
            };
            let da_hash = da_hasher.finalize();
            assert(da_hash == program_output.world_da_hash, 'wrong output hash');

            let mut program_output_array = array![];
            program_output.serialize(ref program_output_array);
            let program_output_hash = poseidon::poseidon_hash_span(program_output_array.span());

            let program_hash = self.config.get_program_hash();
            let fact = poseidon::PoseidonImpl::new()
                .update(program_hash)
                .update(program_output_hash)
                .finalize();
            let fact_registry = IFactRegistryDispatcher {
                contract_address: self.config.get_facts_registry()
            };
            assert(fact_registry.is_valid(fact), 'no state transition proof');

            let mut i = 0;
            loop {
                if i >= new_state.len() {
                    break;
                }
                let base = starknet::storage_base_address_from_felt252(*new_state.at(i).key);
                starknet::storage_write_syscall(
                    0, starknet::storage_address_from_base(base), *new_state.at(i).value
                )
                    .unwrap_syscall();
                i += 2;
            };
            EventEmitter::emit(ref self, StateUpdated { da_hash: da_hash });
        }
    }

    /// Asserts that the current caller can write to the model.
    ///
    /// # Arguments
    ///
    /// * `resource` - The selector of the resource being written to.
    /// * `caller` - The selector of the caller writing.
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
    /// * `resource` - The selector of the resource being verified.
    ///
    /// # Returns
    ///
    /// * `bool` - True if the calling account is the owner of the resource or the owner of the world,
    ///            false otherwise.
    fn is_account_owner(self: @ContractState, resource: felt252) -> bool {
        IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, resource)
            || IWorld::is_owner(self, get_tx_info().unbox().account_contract_address, WORLD)
    }

    #[generate_trait]
    impl Self of SelfTrait {
        /// Write a new model record.
        ///
        /// # Arguments
        ///   * `model` - the model selector
        ///   * `keys` - the list of model keys to identify the record
        ///   * `values` - the field values of the record
        ///   * `layout` - the model layout
        fn _write_model_data(
            ref self: ContractState,
            model: felt252,
            keys: Span<felt252>,
            values: Span<felt252>,
            layout: dojo::database::introspect::Layout
        ) {
            let model_key = poseidon::poseidon_hash_span(keys);
            let mut offset = 0;

            match layout {
                Layout::Fixed(layout) => {
                    Self::_write_fixed_layout(model, model_key, values, ref offset, layout);
                },
                Layout::Struct(layout) => {
                    Self::_write_struct_layout(model, model_key, values, ref offset, layout);
                },
                _ => { panic!("Unexpected layout type for a model."); }
            };
        }

        /// Delete a model record.
        ///
        /// # Arguments
        ///   * `model` - the model selector
        ///   * `keys` - the list of model keys to identify the record
        ///   * `layout` - the model layout
        fn _delete_model_data(
            ref self: ContractState,
            model: felt252,
            keys: Span<felt252>,
            layout: dojo::database::introspect::Layout
        ) {
            let model_key = poseidon::poseidon_hash_span(keys);

            match layout {
                Layout::Fixed(layout) => { Self::_delete_fixed_layout(model, model_key, layout); },
                Layout::Struct(layout) => {
                    Self::_delete_struct_layout(model, model_key, layout);
                },
                _ => { panic!("Unexpected layout type for a model."); }
            };
        }

        /// Read a model record.
        ///
        /// # Arguments
        ///   * `model` - the model selector
        ///   * `keys` - the list of model keys to identify the record
        ///   * `layout` - the model layout
        fn _read_model_data(
            self: @ContractState,
            model: felt252,
            keys: Span<felt252>,
            layout: dojo::database::introspect::Layout
        ) -> Span<felt252> {
            let model_key = poseidon::poseidon_hash_span(keys);
            let mut read_data = ArrayTrait::<felt252>::new();

            match layout {
                Layout::Fixed(layout) => {
                    Self::_read_fixed_layout(model, model_key, ref read_data, layout);
                },
                Layout::Struct(layout) => {
                    Self::_read_struct_layout(model, model_key, ref read_data, layout);
                },
                _ => { panic!("Unexpected layout type for a model."); }
            };

            read_data.span()
        }

        /// Compute the full field key from parent key and current field key.
        fn _field_key(parent_key: felt252, field_key: felt252) -> felt252 {
            poseidon::poseidon_hash_span(array![parent_key, field_key].span())
        }

        /// Append some values to an array.
        ///
        /// # Arguments
        ///  * `dest` - the array to extend
        ///  * `values` - the values to append to the array
        fn _append_array(ref dest: Array<felt252>, values: Span<felt252>) {
            let mut i = 0;
            loop {
                if i >= values.len() {
                    break;
                }
                dest.append(*values.at(i));
                i += 1;
            };
        }

        fn _find_variant_layout(
            variant: felt252, variant_layouts: Span<FieldLayout>
        ) -> Option<Layout> {
            let mut i = 0;
            let layout = loop {
                if i >= variant_layouts.len() {
                    break Option::None;
                }

                let variant_layout = *variant_layouts.at(i);
                if variant == variant_layout.selector {
                    break Option::Some(variant_layout.layout);
                }

                i += 1;
            };

            layout
        }

        /// Write values to the world storage.
        ///
        /// # Arguments
        /// * `model` - the model selector.
        /// * `key` - the object key.
        /// * `values` - the object values.
        /// * `offset` - the start of object values in the `values` parameter.
        /// * `layout` - the object values layout.
        fn _write_layout(
            model: felt252, key: felt252, values: Span<felt252>, ref offset: u32, layout: Layout,
        ) {
            match layout {
                Layout::Fixed(layout) => {
                    Self::_write_fixed_layout(model, key, values, ref offset, layout);
                },
                Layout::Struct(layout) => {
                    Self::_write_struct_layout(model, key, values, ref offset, layout);
                },
                Layout::Array(layout) => {
                    Self::_write_array_layout(model, key, values, ref offset, layout);
                },
                Layout::Tuple(layout) => {
                    Self::_write_tuple_layout(model, key, values, ref offset, layout);
                },
                Layout::ByteArray => {
                    Self::_write_byte_array_layout(model, key, values, ref offset);
                },
                Layout::Enum(layout) => {
                    Self::_write_enum_layout(model, key, values, ref offset, layout);
                }
            }
        }

        /// Write fixed layout model record to the world storage.
        ///
        /// # Arguments
        /// * `model` - the model selector.
        /// * `key` - the model record key.
        /// * `values` - the model record values.
        /// * `offset` - the start of model record values in the `values` parameter.
        /// * `layout` - the model record layout.
        fn _write_fixed_layout(
            model: felt252, key: felt252, values: Span<felt252>, ref offset: u32, layout: Span<u8>
        ) {
            database::set(model, key, values, offset, layout);
            offset += layout.len();
        }

        /// Write array layout model record to the world storage.
        ///
        /// # Arguments
        /// * `model` - the model selector.
        /// * `key` - the model record key.
        /// * `values` - the model record values.
        /// * `offset` - the start of model record values in the `values` parameter.
        /// * `item_layout` - the model record layout (temporary a Span because of type recursion issue).
        fn _write_array_layout(
            model: felt252,
            key: felt252,
            values: Span<felt252>,
            ref offset: u32,
            item_layout: Span<Layout>
        ) {
            assert((values.len() - offset) > 0, 'Invalid values length');

            // first, read array size which is the first felt252 from values
            let array_len = *values.at(offset);
            assert(array_len.into() <= dojo::database::MAX_ARRAY_LENGTH, 'invalid array length');
            let array_len: u32 = array_len.try_into().unwrap();

            // then, write the array size
            database::set(model, key, values, offset, array![251].span());
            offset += 1;

            // and then, write array items
            let item_layout = *item_layout.at(0);

            let mut i = 0;
            loop {
                if i >= array_len {
                    break;
                }
                let key = Self::_field_key(key, i.into());

                Self::_write_layout(model, key, values, ref offset, item_layout);

                i += 1;
            };
        }

        ///
        fn _write_byte_array_layout(
            model: felt252, key: felt252, values: Span<felt252>, ref offset: u32
        ) {
            // The ByteArray internal structure is
            // struct ByteArray {
            //    data: Array<bytes31>,
            //    pending_word: felt252,
            //    pending_word_len: usize,
            // }
            //
            // That means, the length of data to write from 'values' is:
            // 1 + len(data) + 1 + 1 = len(data) + 3
            assert((values.len() - offset) >= MIN_BYTE_ARRAY_SIZE, 'Invalid values length');

            let data_len = *values.at(offset);
            assert(
                data_len.into() <= (dojo::database::MAX_ARRAY_LENGTH - MIN_BYTE_ARRAY_SIZE.into()),
                'invalid array length'
            );

            let array_size: u32 = data_len.try_into().unwrap() + MIN_BYTE_ARRAY_SIZE.into();
            assert((values.len() - offset) >= array_size, 'Invalid values length');

            database::set_array(model, key, values, offset, array_size);
            offset += array_size;
        }

        /// Write struct layout model record to the world storage.
        ///
        /// # Arguments
        /// * `model` - the model selector.
        /// * `key` - the model record key.
        /// * `values` - the model record values.
        /// * `offset` - the start of model record values in the `values` parameter.
        /// * `layout` - list of field layouts.
        fn _write_struct_layout(
            model: felt252,
            key: felt252,
            values: Span<felt252>,
            ref offset: u32,
            layout: Span<FieldLayout>
        ) {
            let mut i = 0;
            loop {
                if i >= layout.len() {
                    break;
                }

                let field_layout = *layout.at(i);
                let field_key = Self::_field_key(key, field_layout.selector);

                Self::_write_layout(model, field_key, values, ref offset, field_layout.layout);

                i += 1;
            }
        }

        /// Write tuple layout model record to the world storage.
        ///
        /// # Arguments
        /// * `model` - the model selector.
        /// * `key` - the model record key.
        /// * `values` - the model record values.
        /// * `offset` - the start of model record values in the `values` parameter.
        /// * `layout` - list of tuple item layouts.
        fn _write_tuple_layout(
            model: felt252,
            key: felt252,
            values: Span<felt252>,
            ref offset: u32,
            layout: Span<Layout>
        ) {
            let mut i = 0;
            loop {
                if i >= layout.len() {
                    break;
                }

                let field_layout = *layout.at(i);
                let key = Self::_field_key(key, i.into());

                Self::_write_layout(model, key, values, ref offset, field_layout);

                i += 1;
            };
        }

        fn _write_enum_layout(
            model: felt252,
            key: felt252,
            values: Span<felt252>,
            ref offset: u32,
            variant_layouts: Span<FieldLayout>
        ) {
            // first, get the variant value from `values``
            let variant = *values.at(offset);
            assert(variant.into() < 256_u256, 'invalid variant value');

            // find the corresponding layout and then write the full variant
            match Self::_find_variant_layout(variant, variant_layouts) {
                Option::Some(layout) => Self::_write_layout(model, key, values, ref offset, layout),
                Option::None => panic!("Unable to find the variant layout")
            };
        }

        /// Delete a fixed layout model record from the world storage.
        ///
        /// # Arguments
        ///   * `model` - the model selector.
        ///   * `key` - the model record key.
        ///   * `layout` - the model layout
        fn _delete_fixed_layout(model: felt252, key: felt252, layout: Span<u8>) {
            database::delete(model, key, layout);
        }

        /// Delete an array layout model record from the world storage.
        ///
        /// # Arguments
        ///   * `model` - the model selector.
        ///   * `key` - the model record key.
        fn _delete_array_layout(model: felt252, key: felt252) {
            // just set the array length to 0
            database::delete(model, key, array![251].span());
        }

        ///
        fn _delete_byte_array_layout(model: felt252, key: felt252) {
            // The ByteArray internal structure is
            // struct ByteArray {
            //    data: Array<bytes31>,
            //    pending_word: felt252,
            //    pending_word_len: usize,
            // }
            //

            // So, just set the 3 first values to 0 (len(data), pending_world and pending_word_len)
            database::delete(model, key, array![251, 251, 251].span());
        }

        /// Delete a model record from the world storage.
        ///
        /// # Arguments
        ///   * `model` - the model selector.
        ///   * `key` - the model record key.
        ///   * `layout` - the model layout
        fn _delete_layout(model: felt252, key: felt252, layout: Layout) {
            match layout {
                Layout::Fixed(layout) => { Self::_delete_fixed_layout(model, key, layout); },
                Layout::Struct(layout) => { Self::_delete_struct_layout(model, key, layout); },
                Layout::Array(_) => { Self::_delete_array_layout(model, key); },
                Layout::Tuple(layout) => { Self::_delete_tuple_layout(model, key, layout); },
                Layout::ByteArray => { Self::_delete_byte_array_layout(model, key); },
                Layout::Enum(layout) => { Self::_delete_enum_layout(model, key, layout); }
            }
        }

        /// Delete a struct layout model record from the world storage.
        ///
        /// # Arguments
        ///   * `model` - the model selector.
        ///   * `key` - the model record key.
        ///   * `layout` - list of field layouts.
        fn _delete_struct_layout(model: felt252, key: felt252, layout: Span<FieldLayout>) {
            let mut i = 0;
            loop {
                if i >= layout.len() {
                    break;
                }

                let field_layout = *layout.at(i);
                let key = Self::_field_key(key, field_layout.selector);

                Self::_delete_layout(model, key, field_layout.layout);

                i += 1;
            }
        }

        /// Delete a tuple layout model record from the world storage.
        ///
        /// # Arguments
        ///   * `model` - the model selector.
        ///   * `key` - the model record key.
        ///   * `layout` - list of tuple item layouts.
        fn _delete_tuple_layout(model: felt252, key: felt252, layout: Span<Layout>) {
            let mut i = 0;
            loop {
                if i >= layout.len() {
                    break;
                }

                let field_layout = *layout.at(i);
                let key = Self::_field_key(key, i.into());

                Self::_delete_layout(model, key, field_layout);

                i += 1;
            }
        }

        fn _delete_enum_layout(model: felt252, key: felt252, variant_layouts: Span<FieldLayout>) {
            // read the variant value first which is the first stored felt252
            let res = database::get(model, key, array![251].span());
            assert(res.len() == 1, 'internal database error');

            let variant = *res.at(0);
            assert(variant.into() < 256_u256, 'invalid variant value');

            // find the corresponding layout and the delete the full variant
            match Self::_find_variant_layout(variant, variant_layouts) {
                Option::Some(layout) => Self::_delete_layout(model, key, layout),
                Option::None => panic!("Unable to find the variant layout")
            };
        }

        /// Read a model record.
        ///
        /// # Arguments
        ///   * `model` - the model selector
        ///   * `key` - model record key.
        ///   * `read_data` - the read data.
        ///   * `layout` - the model layout
        fn _read_layout(
            model: felt252, key: felt252, ref read_data: Array<felt252>, layout: Layout
        ) {
            match layout {
                Layout::Fixed(layout) => Self::_read_fixed_layout(
                    model, key, ref read_data, layout
                ),
                Layout::Struct(layout) => Self::_read_struct_layout(
                    model, key, ref read_data, layout
                ),
                Layout::Array(layout) => Self::_read_array_layout(
                    model, key, ref read_data, layout
                ),
                Layout::Tuple(layout) => Self::_read_tuple_layout(
                    model, key, ref read_data, layout
                ),
                Layout::ByteArray => Self::_read_byte_array_layout(model, key, ref read_data),
                Layout::Enum(layout) => Self::_read_enum_layout(model, key, ref read_data, layout),
            };
        }

        /// Read a fixed layout model record.
        ///
        /// # Arguments
        ///   * `model` - the model selector
        ///   * `key` - model record key.
        ///   * `read_data` - the read data.
        ///   * `layout` - the model layout
        fn _read_fixed_layout(
            model: felt252, key: felt252, ref read_data: Array<felt252>, layout: Span<u8>
        ) {
            let data = database::get(model, key, layout);
            Self::_append_array(ref read_data, data);
        }

        /// Read an array layout model record.
        ///
        /// # Arguments
        ///   * `model` - the model selector
        ///   * `key` - model record key.
        ///   * `read_data` - the read data.
        ///   * `layout` - the array item layout
        fn _read_array_layout(
            model: felt252, key: felt252, ref read_data: Array<felt252>, layout: Span<Layout>
        ) {
            // read number of array items
            let res = database::get(model, key, array![251].span());
            assert(res.len() == 1, 'internal database error');

            let array_len = *res.at(0);
            assert(array_len.into() <= dojo::database::MAX_ARRAY_LENGTH, 'invalid array length');

            read_data.append(array_len);

            let item_layout = *layout.at(0);
            let array_len: u32 = array_len.try_into().unwrap();

            let mut i = 0;
            loop {
                if i >= array_len {
                    break;
                }

                let field_key = Self::_field_key(key, i.into());
                Self::_read_layout(model, field_key, ref read_data, item_layout);

                i += 1;
            };
        }

        ///
        fn _read_byte_array_layout(model: felt252, key: felt252, ref read_data: Array<felt252>) {
            // The ByteArray internal structure is
            // struct ByteArray {
            //    data: Array<bytes31>,
            //    pending_word: felt252,
            //    pending_word_len: usize,
            // }
            //
            // So, read the length of data and compute the full size to read

            let res = database::get(model, key, array![251].span());
            assert(res.len() == 1, 'internal database error');

            let data_len = *res.at(0);
            assert(
                data_len.into() <= (dojo::database::MAX_ARRAY_LENGTH - MIN_BYTE_ARRAY_SIZE.into()),
                'invalid array length'
            );

            let array_size: u32 = data_len.try_into().unwrap() + MIN_BYTE_ARRAY_SIZE;

            let data = database::get_array(model, key, array_size);

            Self::_append_array(ref read_data, data);
        }

        /// Read a struct layout model record.
        ///
        /// # Arguments
        ///   * `model` - the model selector
        ///   * `key` - model record key.
        ///   * `read_data` - the read data.
        ///   * `layout` - the list of field layouts.
        fn _read_struct_layout(
            model: felt252, key: felt252, ref read_data: Array<felt252>, layout: Span<FieldLayout>
        ) {
            let mut i = 0;
            loop {
                if i >= layout.len() {
                    break;
                }

                let field_layout = *layout.at(i);
                let field_key = Self::_field_key(key, field_layout.selector);

                Self::_read_layout(model, field_key, ref read_data, field_layout.layout);

                i += 1;
            }
        }

        /// Read a tuple layout model record.
        ///
        /// # Arguments
        ///   * `model` - the model selector
        ///   * `key` - model record key.
        ///   * `read_data` - the read data.
        ///   * `layout` - the tuple item layouts
        fn _read_tuple_layout(
            model: felt252, key: felt252, ref read_data: Array<felt252>, layout: Span<Layout>
        ) {
            let mut i = 0;
            loop {
                if i >= layout.len() {
                    break;
                }

                let field_layout = *layout.at(i);
                let field_key = Self::_field_key(key, i.into());
                Self::_read_layout(model, field_key, ref read_data, field_layout);

                i += 1;
            };
        }

        fn _read_enum_layout(
            model: felt252,
            key: felt252,
            ref read_data: Array<felt252>,
            variant_layouts: Span<FieldLayout>
        ) {
            // read the variant value first, which is the first element of the tuple
            // (because an enum is stored as a tuple).
            let variant_key = Self::_field_key(key, 0);
            let res = database::get(model, variant_key, array![8].span());
            assert(res.len() == 1, 'internal database error');

            let variant = *res.at(0);
            assert(variant.into() < 256_u256, 'invalid variant value');

            // find the corresponding layout and the read the full variant
            match Self::_find_variant_layout(variant, variant_layouts) {
                Option::Some(layout) => Self::_read_layout(model, key, ref read_data, layout),
                Option::None => panic!("Unable to find the variant layout")
            };
        }
    }
}
