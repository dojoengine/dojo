use core::option::OptionTrait;
use core::traits::{Into, TryInto};
use starknet::{ContractAddress, ClassHash, storage_access::StorageBaseAddress, SyscallResult};

use dojo::model::{ModelIndex, ResourceMetadata};
use dojo::model::{Layout};
use dojo::utils::bytearray_hash;

#[derive(Drop, starknet::Store, Serde, Default, Debug)]
pub enum Resource {
    Model: (ClassHash, ContractAddress),
    Contract: (ClassHash, ContractAddress),
    Namespace,
    World,
    #[default]
    Unregistered,
}

#[starknet::interface]
pub trait IWorld<T> {
    fn metadata(self: @T, resource_selector: felt252) -> ResourceMetadata;
    fn set_metadata(ref self: T, metadata: ResourceMetadata);

    fn register_namespace(ref self: T, namespace: ByteArray);

    fn register_model(ref self: T, class_hash: ClassHash);
    fn upgrade_model(ref self: T, class_hash: ClassHash);

    fn deploy_contract(ref self: T, salt: felt252, class_hash: ClassHash) -> ContractAddress;
    fn upgrade_contract(ref self: T, selector: felt252, class_hash: ClassHash) -> ClassHash;
    fn init_contract(ref self: T, selector: felt252, init_calldata: Span<felt252>);

    fn uuid(ref self: T) -> usize;
    fn emit(self: @T, keys: Array<felt252>, values: Span<felt252>);

    fn entity(
        self: @T, model_selector: felt252, index: ModelIndex, layout: Layout
    ) -> Span<felt252>;
    fn set_entity(
        ref self: T,
        model_selector: felt252,
        index: ModelIndex,
        values: Span<felt252>,
        layout: Layout
    );
    fn delete_entity(ref self: T, model_selector: felt252, index: ModelIndex, layout: Layout);

    fn base(self: @T) -> ClassHash;
    fn resource(self: @T, selector: felt252) -> Resource;

    /// In Dojo, there are 2 levels of authorization: `owner` and `writer`.
    /// Only accounts can own a resource while any contract can write to a resource,
    /// as soon as it has granted the write access from an owner of the resource.
    fn is_owner(self: @T, resource: felt252, address: ContractAddress) -> bool;
    fn grant_owner(ref self: T, resource: felt252, address: ContractAddress);
    fn revoke_owner(ref self: T, resource: felt252, address: ContractAddress);

    fn is_writer(self: @T, resource: felt252, contract: ContractAddress) -> bool;
    fn grant_writer(ref self: T, resource: felt252, contract: ContractAddress);
    fn revoke_writer(ref self: T, resource: felt252, contract: ContractAddress);
}

#[starknet::interface]
#[cfg(target: "test")]
pub trait IWorldTest<T> {
    fn set_entity_test(
        ref self: T,
        model_selector: felt252,
        index: ModelIndex,
        values: Span<felt252>,
        layout: Layout
    );

    fn delete_entity_test(ref self: T, model_selector: felt252, index: ModelIndex, layout: Layout);
}

#[starknet::interface]
pub trait IUpgradeableWorld<T> {
    fn upgrade(ref self: T, new_class_hash: ClassHash);
}

#[starknet::interface]
pub trait IWorldProvider<T> {
    fn world(self: @T) -> IWorldDispatcher;
}

#[starknet::contract]
pub mod world {
    use core::array::{ArrayTrait, SpanTrait};
    use core::box::BoxTrait;
    use core::hash::{HashStateExTrait, HashStateTrait};
    use core::num::traits::Zero;
    use core::option::OptionTrait;
    use core::pedersen::PedersenTrait;
    use core::serde::Serde;
    use core::to_byte_array::FormatAsByteArray;
    use core::traits::TryInto;
    use core::traits::Into;
    use core::panic_with_felt252;
    use core::panics::panic_with_byte_array;

    use starknet::event::EventEmitter;
    use starknet::{
        contract_address_const, get_caller_address, get_contract_address, get_tx_info, ClassHash,
        ContractAddress, syscalls::{deploy_syscall, emit_event_syscall, replace_class_syscall},
        SyscallResult, SyscallResultTrait, storage::Map,
    };
    pub use starknet::storage::{
        StorageMapReadAccess, StorageMapWriteAccess, StoragePointerReadAccess,
        StoragePointerWriteAccess
    };

    use dojo::world::errors;
    use dojo::world::config::{Config, IConfig};
    use dojo::contract::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
    use dojo::contract::{IContractDispatcher, IContractDispatcherTrait};
    use dojo::world::update::{
        IUpgradeableState, IFactRegistryDispatcher, IFactRegistryDispatcherTrait, StorageUpdate,
        ProgramOutput
    };
    use dojo::model::{
        Model, IModelDispatcher, IModelDispatcherTrait, Layout, ResourceMetadata,
        ResourceMetadataTrait, metadata
    };
    use dojo::storage;
    use dojo::utils::{entity_id_from_keys, bytearray_hash};

    use super::{
        ModelIndex, IWorldDispatcher, IWorldDispatcherTrait, IWorld, IUpgradeableWorld, Resource
    };

    const WORLD: felt252 = 0;

    const DOJO_INIT_SELECTOR: felt252 = selector!("dojo_init");

    component!(path: Config, storage: config, event: ConfigEvent);

    #[abi(embed_v0)]
    impl ConfigImpl = Config::ConfigImpl<ContractState>;
    impl ConfigInternalImpl = Config::InternalImpl<ContractState>;

    #[event]
    #[derive(Drop, starknet::Event)]
    pub enum Event {
        WorldSpawned: WorldSpawned,
        ContractDeployed: ContractDeployed,
        ContractUpgraded: ContractUpgraded,
        ContractInitialized: ContractInitialized,
        WorldUpgraded: WorldUpgraded,
        MetadataUpdate: MetadataUpdate,
        NamespaceRegistered: NamespaceRegistered,
        ModelRegistered: ModelRegistered,
        ModelUpgraded: ModelUpgraded,
        StoreSetRecord: StoreSetRecord,
        StoreUpdateRecord: StoreUpdateRecord,
        StoreUpdateMember: StoreUpdateMember,
        StoreDelRecord: StoreDelRecord,
        WriterUpdated: WriterUpdated,
        OwnerUpdated: OwnerUpdated,
        ConfigEvent: Config::Event,
        StateUpdated: StateUpdated
    }

    #[derive(Drop, starknet::Event)]
    pub struct StateUpdated {
        pub da_hash: felt252,
    }

    #[derive(Drop, starknet::Event)]
    pub struct WorldSpawned {
        pub address: ContractAddress,
        pub creator: ContractAddress
    }

    #[derive(Drop, starknet::Event)]
    pub struct WorldUpgraded {
        pub class_hash: ClassHash,
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct ContractDeployed {
        pub salt: felt252,
        pub class_hash: ClassHash,
        pub address: ContractAddress,
        pub namespace: ByteArray,
        pub name: ByteArray
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct ContractUpgraded {
        pub class_hash: ClassHash,
        pub address: ContractAddress,
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct ContractInitialized {
        pub selector: felt252,
        pub init_calldata: Span<felt252>,
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct MetadataUpdate {
        pub resource: felt252,
        pub uri: ByteArray
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct NamespaceRegistered {
        pub namespace: ByteArray,
        pub hash: felt252
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct ModelRegistered {
        pub name: ByteArray,
        pub namespace: ByteArray,
        pub class_hash: ClassHash,
        pub address: ContractAddress,
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct ModelUpgraded {
        pub name: ByteArray,
        pub namespace: ByteArray,
        pub class_hash: ClassHash,
        pub prev_class_hash: ClassHash,
        pub address: ContractAddress,
        pub prev_address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreSetRecord {
        pub table: felt252,
        pub keys: Span<felt252>,
        pub values: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreUpdateRecord {
        pub table: felt252,
        pub entity_id: felt252,
        pub values: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreUpdateMember {
        pub table: felt252,
        pub entity_id: felt252,
        pub member_selector: felt252,
        pub values: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreDelRecord {
        pub table: felt252,
        pub entity_id: felt252,
    }

    #[derive(Drop, starknet::Event)]
    pub struct WriterUpdated {
        pub resource: felt252,
        pub contract: ContractAddress,
        pub value: bool
    }

    #[derive(Drop, starknet::Event)]
    pub struct OwnerUpdated {
        pub address: ContractAddress,
        pub resource: felt252,
        pub value: bool,
    }

    #[storage]
    struct Storage {
        contract_base: ClassHash,
        nonce: usize,
        models_salt: usize,
        resources: Map::<felt252, Resource>,
        owners: Map::<(felt252, ContractAddress), bool>,
        writers: Map::<(felt252, ContractAddress), bool>,
        #[substorage(v0)]
        config: Config::Storage,
        initialized_contract: Map::<felt252, bool>,
    }

    #[generate_trait]
    impl ResourceIsNoneImpl of ResourceIsNoneTrait {
        fn is_unregistered(self: @Resource) -> bool {
            match self {
                Resource::Unregistered => true,
                _ => false
            }
        }
    }

    #[constructor]
    fn constructor(ref self: ContractState, contract_base: ClassHash) {
        let creator = starknet::get_tx_info().unbox().account_contract_address;
        self.contract_base.write(contract_base);

        self.resources.write(WORLD, Resource::World);
        self
            .resources
            .write(
                Model::<ResourceMetadata>::selector(),
                Resource::Model((metadata::initial_class_hash(), metadata::initial_address()))
            );
        self.owners.write((WORLD, creator), true);

        let dojo_namespace_hash = bytearray_hash(@"__DOJO__");

        self.resources.write(dojo_namespace_hash, Resource::Namespace);
        self.owners.write((dojo_namespace_hash, creator), true);

        self.config.initializer(creator);

        EventEmitter::emit(ref self, WorldSpawned { address: get_contract_address(), creator });
    }

    #[cfg(target: "test")]
    #[abi(embed_v0)]
    impl WorldTestImpl of super::IWorldTest<ContractState> {
        fn set_entity_test(
            ref self: ContractState,
            model_selector: felt252,
            index: ModelIndex,
            values: Span<felt252>,
            layout: Layout
        ) {
            self.set_entity_internal(model_selector, index, values, layout);
        }

        fn delete_entity_test(
            ref self: ContractState, model_selector: felt252, index: ModelIndex, layout: Layout
        ) {
            self.delete_entity_internal(model_selector, index, layout);
        }
    }

    #[abi(embed_v0)]
    impl World of IWorld<ContractState> {
        /// Returns the metadata of the resource.
        ///
        /// # Arguments
        ///
        /// `resource_selector` - The resource selector.
        fn metadata(self: @ContractState, resource_selector: felt252) -> ResourceMetadata {
            let mut values = self
                .read_model_entity(
                    Model::<ResourceMetadata>::selector(),
                    entity_id_from_keys([resource_selector].span()),
                    Model::<ResourceMetadata>::layout()
                );

            ResourceMetadataTrait::from_values(resource_selector, ref values)
        }

        /// Sets the metadata of the resource.
        ///
        /// # Arguments
        ///
        /// `metadata` - The metadata content for the resource.
        fn set_metadata(ref self: ContractState, metadata: ResourceMetadata) {
            self.assert_caller_is_resource_owner(metadata.resource_id);

            self
                .write_model_entity(
                    metadata.instance_selector(),
                    metadata.entity_id(),
                    metadata.values(),
                    metadata.instance_layout()
                );

            EventEmitter::emit(
                ref self,
                MetadataUpdate { resource: metadata.resource_id, uri: metadata.metadata_uri }
            );
        }

        /// Checks if the provided account is an owner of the resource.
        ///
        /// # Arguments
        ///
        /// * `resource` - The resource.
        /// * `address` - The contract address.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the address is an owner of the resource, false otherwise.
        fn is_owner(self: @ContractState, resource: felt252, address: ContractAddress) -> bool {
            self.owners.read((resource, address))
        }

        /// Grants ownership of the resource to the address.
        /// Can only be called by an existing owner or the world admin.
        ///
        /// Note that this resource must have been registered to the world first.
        ///
        /// # Arguments
        ///
        /// * `resource` - The resource.
        /// * `address` - The contract address.
        fn grant_owner(ref self: ContractState, resource: felt252, address: ContractAddress) {
            if self.resources.read(resource).is_unregistered() {
                panic_with_byte_array(@errors::resource_not_registered(resource));
            }

            self.assert_caller_is_resource_owner(resource);

            self.owners.write((resource, address), true);

            EventEmitter::emit(ref self, OwnerUpdated { address, resource, value: true });
        }

        /// Revokes owner permission to the contract for the model.
        /// Can only be called by an existing owner or the world admin.
        ///
        /// Note that this resource must have been registered to the world first.
        ///
        /// # Arguments
        ///
        /// * `resource` - The resource.
        /// * `address` - The contract address.
        fn revoke_owner(ref self: ContractState, resource: felt252, address: ContractAddress) {
            if self.resources.read(resource).is_unregistered() {
                panic_with_byte_array(@errors::resource_not_registered(resource));
            }

            self.assert_caller_is_resource_owner(resource);

            self.owners.write((resource, address), false);

            EventEmitter::emit(ref self, OwnerUpdated { address, resource, value: false });
        }

        /// Checks if the provided contract is a writer of the resource.
        ///
        /// # Arguments
        ///
        /// * `resource` - The hash of the resource name.
        /// * `contract` - The name of the contract.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the contract is a writer of the resource, false otherwise
        fn is_writer(self: @ContractState, resource: felt252, contract: ContractAddress) -> bool {
            self.writers.read((resource, contract))
        }

        /// Grants writer permission to the contract for the resource.
        /// Can only be called by an existing resource owner or the world admin.
        ///
        /// Note that this resource must have been registered to the world first.
        ///
        /// # Arguments
        ///
        /// * `resource` - The hash of the resource name.
        /// * `contract` - The name of the contract.
        fn grant_writer(ref self: ContractState, resource: felt252, contract: ContractAddress) {
            if self.resources.read(resource).is_unregistered() {
                panic_with_byte_array(@errors::resource_not_registered(resource));
            }

            self.assert_caller_is_resource_owner(resource);

            self.writers.write((resource, contract), true);

            EventEmitter::emit(ref self, WriterUpdated { resource, contract, value: true });
        }

        /// Revokes writer permission to the contract for the model.
        /// Can only be called by an existing model owner or the world admin.
        ///
        /// Note that this resource must have been registered to the world first.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model.
        /// * `contract` - The name of the contract.
        fn revoke_writer(ref self: ContractState, resource: felt252, contract: ContractAddress) {
            if self.resources.read(resource).is_unregistered() {
                panic_with_byte_array(@errors::resource_not_registered(resource));
            }

            self.assert_caller_is_resource_owner(resource);

            self.writers.write((resource, contract), false);

            EventEmitter::emit(ref self, WriterUpdated { resource, contract, value: false });
        }

        /// Registers a model in the world. If the model is already registered,
        /// the implementation will be updated.
        ///
        /// # Arguments
        ///
        /// * `class_hash` - The class hash of the model to be registered.
        fn register_model(ref self: ContractState, class_hash: ClassHash) {
            let caller = get_caller_address();

            let salt = self.models_salt.read();
            let (address, name, selector, namespace, namespace_hash) =
                dojo::model::deploy_and_get_metadata(
                salt.into(), class_hash
            )
                .unwrap_syscall();
            self.models_salt.write(salt + 1);

            if selector.is_zero() {
                panic_with_byte_array(@errors::invalid_resource_selector(selector));
            }

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_namespace_write_access(@namespace, namespace_hash);

            let model = self.resources.read(selector);
            if !model.is_unregistered() {
                panic_with_byte_array(@errors::model_already_registered(@namespace, @name));
            }

            self.resources.write(selector, Resource::Model((class_hash, address)));
            self.owners.write((selector, caller), true);

            EventEmitter::emit(ref self, ModelRegistered { name, namespace, address, class_hash });
        }

        fn upgrade_model(ref self: ContractState, class_hash: ClassHash) {
            let caller = get_caller_address();

            let salt = self.models_salt.read();
            let (address, name, selector, namespace, namespace_hash) =
                dojo::model::deploy_and_get_metadata(
                salt.into(), class_hash
            )
                .unwrap_syscall();
            self.models_salt.write(salt + 1);

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_namespace_write_access(@namespace, namespace_hash);

            if selector.is_zero() {
                panic_with_byte_array(@errors::invalid_resource_selector(selector));
            }

            let mut prev_class_hash = core::num::traits::Zero::<ClassHash>::zero();
            let mut prev_address = core::num::traits::Zero::<ContractAddress>::zero();

            match self.resources.read(selector) {
                // If model is already registered, validate permission to update.
                Resource::Model((
                    model_hash, model_address
                )) => {
                    if !self.is_owner(selector, caller) {
                        panic_with_byte_array(@errors::not_owner_upgrade(caller, selector));
                    }

                    prev_class_hash = model_hash;
                    prev_address = model_address;
                },
                Resource::Unregistered => {
                    panic_with_byte_array(@errors::model_not_registered(@namespace, @name))
                },
                _ => panic_with_byte_array(
                    @errors::resource_conflict(@format!("{}-{}", namespace, name), @"model")
                )
            };

            self.resources.write(selector, Resource::Model((class_hash, address)));

            EventEmitter::emit(
                ref self,
                ModelUpgraded {
                    name, namespace, prev_address, address, class_hash, prev_class_hash
                }
            );
        }

        /// Registers a namespace in the world.
        ///
        /// # Arguments
        ///
        /// * `namespace` - The name of the namespace to be registered.
        fn register_namespace(ref self: ContractState, namespace: ByteArray) {
            let caller = get_caller_address();

            let hash = bytearray_hash(@namespace);

            match self.resources.read(hash) {
                Resource::Namespace => panic_with_byte_array(
                    @errors::namespace_already_registered(@namespace)
                ),
                Resource::Unregistered => {
                    self.resources.write(hash, Resource::Namespace);
                    self.owners.write((hash, caller), true);

                    EventEmitter::emit(ref self, NamespaceRegistered { namespace, hash });
                },
                _ => {
                    panic_with_byte_array(@errors::resource_conflict(@namespace, @"namespace"));
                }
            };
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
            ref self: ContractState, salt: felt252, class_hash: ClassHash,
        ) -> ContractAddress {
            let caller = get_caller_address();

            let (contract_address, _) = deploy_syscall(
                self.contract_base.read(), salt, [].span(), false
            )
                .unwrap_syscall();
            let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address };
            upgradeable_dispatcher.upgrade(class_hash);

            let dispatcher = IContractDispatcher { contract_address };
            let namespace = dispatcher.namespace();
            let name = dispatcher.contract_name();
            let namespace_hash = dispatcher.namespace_hash();

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_namespace_write_access(@namespace, namespace_hash);

            let selector = dispatcher.selector();
            self.owners.write((selector, caller), true);
            self.resources.write(selector, Resource::Contract((class_hash, contract_address)));

            EventEmitter::emit(
                ref self,
                ContractDeployed { salt, class_hash, address: contract_address, namespace, name }
            );

            contract_address
        }

        /// Upgrades an already deployed contract associated with the world.
        ///
        /// # Arguments
        ///
        /// * `selector` - The selector of the contract to upgrade.
        /// * `class_hash` - The class hash of the contract.
        ///
        /// # Returns
        ///
        /// * `ClassHash` - The new class hash of the contract.
        fn upgrade_contract(
            ref self: ContractState, selector: felt252, class_hash: ClassHash
        ) -> ClassHash {
            self.assert_caller_is_resource_owner(selector);

            if let Resource::Contract((_, contract_address)) = self.resources.read(selector) {
                IUpgradeableDispatcher { contract_address }.upgrade(class_hash);
                EventEmitter::emit(
                    ref self, ContractUpgraded { class_hash, address: contract_address }
                );
                class_hash
            } else {
                panic_with_byte_array(@errors::invalid_resource_selector(selector))
            }
        }

        /// Initializes a contract associated with the world.
        ///
        /// # Arguments
        ///
        /// * `selector` - The selector of the contract to initialize.
        /// * `init_calldata` - Calldata used to initialize the contract.
        fn init_contract(ref self: ContractState, selector: felt252, init_calldata: Span<felt252>) {
            if let Resource::Contract((_, contract_address)) = self.resources.read(selector) {
                let caller = get_caller_address();

                let dispatcher = IContractDispatcher { contract_address };
                let tag = dispatcher.tag();

                if self.initialized_contract.read(selector) {
                    panic_with_byte_array(@errors::contract_already_initialized(@tag));
                } else {
                    if !self.is_owner(selector, caller) {
                        panic_with_byte_array(@errors::not_owner_init(@tag, caller));
                    }

                    // For the init, to ensure only the world can call the init function,
                    // the verification is done in the init function of the contract:
                    // `crates/dojo-lang/src/contract.rs#L140`
                    // `crates/dojo-lang/src/contract.rs#L331`

                    starknet::syscalls::call_contract_syscall(
                        contract_address, DOJO_INIT_SELECTOR, init_calldata
                    )
                        .unwrap_syscall();

                    self.initialized_contract.write(selector, true);

                    EventEmitter::emit(ref self, ContractInitialized { selector, init_calldata });
                }
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{selector}"), @"contract")
                );
            }
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

        /// Gets the values of a model record/entity/member.
        /// Returns a zero initialized model value if the record/entity/member has not been set.
        ///
        /// # Arguments
        ///
        /// * `model_selector` - The selector of the model to be retrieved.
        /// * `index` - The index of the record/entity/member to read.
        /// * `layout` - The memory layout of the model.
        ///
        /// # Returns
        ///
        /// * `Span<felt252>` - The serialized value of the model, zero initialized if not set.
        fn entity(
            self: @ContractState, model_selector: felt252, index: ModelIndex, layout: Layout
        ) -> Span<felt252> {
            match index {
                ModelIndex::Keys(keys) => {
                    let entity_id = entity_id_from_keys(keys);
                    self.read_model_entity(model_selector, entity_id, layout)
                },
                ModelIndex::Id(entity_id) => {
                    self.read_model_entity(model_selector, entity_id, layout)
                },
                ModelIndex::MemberId((
                    entity_id, member_id
                )) => { self.read_model_member(model_selector, entity_id, member_id, layout) }
            }
        }

        /// Sets the model value for a model record/entity/member.
        ///
        /// # Arguments
        ///
        /// * `model_selector` - The selector of the model to be set.
        /// * `index` - The index of the record/entity/member to write.
        /// * `values` - The value to be set, serialized using the model layout format.
        /// * `layout` - The memory layout of the model.
        fn set_entity(
            ref self: ContractState,
            model_selector: felt252,
            index: ModelIndex,
            values: Span<felt252>,
            layout: Layout
        ) {
            self.assert_caller_model_write_access(model_selector);
            self.set_entity_internal(model_selector, index, values, layout);
        }

        /// Deletes a record/entity of a model..
        /// Deleting is setting all the values to 0 in the given layout.
        ///
        /// # Arguments
        ///
        /// * `model_selector` - The selector of the model to be deleted.
        /// * `index` - The index of the record/entity to delete.
        /// * `layout` - The memory layout of the model.
        fn delete_entity(
            ref self: ContractState, model_selector: felt252, index: ModelIndex, layout: Layout
        ) {
            self.assert_caller_model_write_access(model_selector);
            self.delete_entity_internal(model_selector, index, layout);
        }

        /// Gets the base contract class hash.
        ///
        /// # Returns
        ///
        /// * `ClassHash` - The class_hash of the contract_base contract.
        fn base(self: @ContractState) -> ClassHash {
            self.contract_base.read()
        }

        /// Gets resource data from its selector.
        ///
        /// # Arguments
        ///   * `selector` - the resource selector
        ///
        /// # Returns
        ///   * `Resource` - the resource data associated with the selector.
        fn resource(self: @ContractState, selector: felt252) -> Resource {
            self.resources.read(selector)
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

            if !self.is_caller_world_owner() {
                panic_with_byte_array(@errors::not_owner_upgrade(get_caller_address(), WORLD));
            }

            // upgrade to new_class_hash
            replace_class_syscall(new_class_hash).unwrap();

            // emit Upgrade Event
            EventEmitter::emit(ref self, WorldUpgraded { class_hash: new_class_hash });
        }
    }

    #[abi(embed_v0)]
    impl UpgradeableState of IUpgradeableState<ContractState> {
        fn upgrade_state(
            ref self: ContractState,
            new_state: Span<StorageUpdate>,
            program_output: ProgramOutput,
            program_hash: felt252
        ) {
            if !self.is_caller_world_owner() {
                panic_with_byte_array(
                    @errors::no_world_owner(get_caller_address(), @"upgrade state")
                );
            }

            let mut da_hasher = PedersenTrait::new(0);
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

            assert(
                program_hash == self.config.get_differ_program_hash()
                    || program_hash == self.config.get_merger_program_hash(),
                'wrong program hash'
            );

            let mut program_output_array = array![];
            program_output.serialize(ref program_output_array);
            let program_output_hash = core::poseidon::poseidon_hash_span(
                program_output_array.span()
            );

            let fact = core::poseidon::PoseidonImpl::new()
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
                let base = starknet::storage_access::storage_base_address_from_felt252(
                    *new_state.at(i).key
                );
                starknet::syscalls::storage_write_syscall(
                    0,
                    starknet::storage_access::storage_address_from_base(base),
                    *new_state.at(i).value
                )
                    .unwrap_syscall();
                i += 1;
            };
            EventEmitter::emit(ref self, StateUpdated { da_hash: da_hash });
        }
    }

    #[generate_trait]
    impl SelfImpl of SelfTrait {
        #[inline(always)]
        /// Indicates if the caller is the owner of the world.
        fn is_caller_world_owner(self: @ContractState) -> bool {
            self.is_owner(WORLD, get_caller_address())
        }

        /// Panics if the caller is NOT an owner of the resource.
        ///
        /// # Arguments
        ///   * `resource_selector` - the selector of the resource.
        #[inline(always)]
        fn assert_caller_is_resource_owner(self: @ContractState, resource_selector: felt252) {
            let caller = get_caller_address();

            if self.is_owner(resource_selector, caller) {
                return;
            }

            if self.is_caller_world_owner() {
                return;
            }

            panic_with_byte_array(@errors::not_owner(caller, resource_selector));
        }

        /// Panics if the caller has NOT the writer role on the model.
        ///
        /// # Arguments
        ///   * `model_selector` - the selector of the model.
        #[inline(always)]
        fn assert_caller_model_write_access(self: @ContractState, model_selector: felt252) {
            let caller = get_caller_address();

            // Must have owner or writer role on the namespace or on the model.
            match self.resources.read(model_selector) {
                Resource::Model((
                    _, model_address
                )) => {
                    let model = IModelDispatcher { contract_address: model_address };
                    let namespace_selector = model.namespace_hash();

                    // - use several single if because it seems more efficient than a big one with
                    // several conditions.
                    // - sort conditions by order of probability so once a condition is met, the
                    // function returns.
                    if self.is_writer(namespace_selector, caller) {
                        return;
                    }
                    if self.is_writer(model_selector, caller) {
                        return;
                    }
                    if self.is_owner(namespace_selector, caller) {
                        return;
                    }
                    if self.is_owner(model_selector, caller) {
                        return;
                    }
                    if self.is_caller_world_owner() {
                        return;
                    }

                    let model_tag = model.tag();
                    let d = IContractDispatcher { contract_address: caller };

                    // If the caller is not a dojo contract, the `d.selector()` will fail. In the
                    // future use the SRC5 to first query the contract to see if it implements the
                    // `IContract` interface.
                    // For now, we just assume that the caller is a dojo contract as it's 100% of
                    // the dojo use cases at the moment.
                    if let Resource::Contract((_, contract_address)) = self
                        .resources
                        .read(d.selector()) {
                        let d = IContractDispatcher { contract_address };
                        panic_with_byte_array(
                            @errors::no_write_access_with_tags(
                                @d.tag(), @"model (or it's namespace)", @model_tag
                            )
                        );
                    } else {
                        panic_with_byte_array(@errors::no_model_write_access(@model_tag, caller));
                    }
                },
                Resource::Unregistered => {
                    panic_with_byte_array(@errors::resource_not_registered(model_selector));
                },
                _ => panic_with_byte_array(
                    @errors::resource_conflict(@format!("{}", model_selector), @"model")
                )
            }
        }

        /// Panics if the caller has NOT the writer role on the namespace.
        ///
        /// # Arguments
        ///   * `namespace` - the namespace name.
        ///   * `namespace_hash` - the hash of the namespace.
        #[inline(always)]
        fn assert_caller_namespace_write_access(
            self: @ContractState, namespace: @ByteArray, namespace_hash: felt252
        ) {
            let caller = get_caller_address();

            if self.is_writer(namespace_hash, caller) {
                return;
            }
            if self.is_owner(namespace_hash, caller) {
                return;
            }
            if self.is_caller_world_owner() {
                return;
            }

            // We know it's an account and return the explicit error message as no tag will match
            // the account.
            if caller == get_tx_info().account_contract_address {
                panic_with_byte_array(@errors::no_namespace_write_access(caller, namespace));
            }

            // If the caller is not a dojo contract, the `d.selector()` will fail. In the future use
            // the SRC5 to first query the contract to see if it implements the `IContract`
            // interface.
            // For now, we just assume that the caller is a dojo contract as it's 100% of the dojo
            // use cases at the moment.
            let d = IContractDispatcher { contract_address: caller };

            if let Resource::Contract((_, contract_address)) = self.resources.read(d.selector()) {
                let d = IContractDispatcher { contract_address };
                panic_with_byte_array(
                    @errors::no_write_access_with_tags(@d.tag(), @"namespace", namespace)
                );
            } else {
                // This is in theory unreachable code as the contract call syscall made by the
                // dispatcher will panic. Which may lead to a bad user experience in testing as the
                // error will be something like "CONTRACT_NOT_DEPLOYED".
                panic_with_byte_array(@errors::no_namespace_write_access(caller, namespace));
            }
        }

        /// Indicates if the provided namespace is already registered
        ///
        /// # Arguments
        ///   * `namespace_hash` - the hash of the namespace.
        #[inline(always)]
        fn is_namespace_registered(self: @ContractState, namespace_hash: felt252) -> bool {
            match self.resources.read(namespace_hash) {
                Resource::Namespace => true,
                _ => false
            }
        }

        /// Sets the model value for a model record/entity/member.
        ///
        /// # Arguments
        ///
        /// * `model_selector` - The selector of the model to be set.
        /// * `index` - The index of the record/entity/member to write.
        /// * `values` - The value to be set, serialized using the model layout format.
        /// * `layout` - The memory layout of the model.
        fn set_entity_internal(
            ref self: ContractState,
            model_selector: felt252,
            index: ModelIndex,
            values: Span<felt252>,
            layout: Layout
        ) {
            match index {
                ModelIndex::Keys(keys) => {
                    let entity_id = entity_id_from_keys(keys);
                    self.write_model_entity(model_selector, entity_id, values, layout);
                    EventEmitter::emit(
                        ref self, StoreSetRecord { table: model_selector, keys, values }
                    );
                },
                ModelIndex::Id(entity_id) => {
                    self.write_model_entity(model_selector, entity_id, values, layout);
                    EventEmitter::emit(
                        ref self, StoreUpdateRecord { table: model_selector, entity_id, values }
                    );
                },
                ModelIndex::MemberId((
                    entity_id, member_selector
                )) => {
                    self
                        .write_model_member(
                            model_selector, entity_id, member_selector, values, layout
                        );
                    EventEmitter::emit(
                        ref self,
                        StoreUpdateMember {
                            table: model_selector, entity_id, member_selector, values
                        }
                    );
                }
            }
        }

        /// Deletes an entity for the given model, setting all the values to 0 in the given layout.
        ///
        /// # Arguments
        ///
        /// * `model_selector` - The selector of the model to be deleted.
        /// * `index` - The index of the record/entity to delete.
        /// * `layout` - The memory layout of the model.
        fn delete_entity_internal(
            ref self: ContractState, model_selector: felt252, index: ModelIndex, layout: Layout
        ) {
            match index {
                ModelIndex::Keys(keys) => {
                    let entity_id = entity_id_from_keys(keys);
                    self.delete_model_entity(model_selector, entity_id, layout);
                    EventEmitter::emit(
                        ref self, StoreDelRecord { table: model_selector, entity_id }
                    );
                },
                ModelIndex::Id(entity_id) => {
                    self.delete_model_entity(model_selector, entity_id, layout);
                    EventEmitter::emit(
                        ref self, StoreDelRecord { table: model_selector, entity_id }
                    );
                },
                ModelIndex::MemberId(_) => { panic_with_felt252(errors::DELETE_ENTITY_MEMBER); }
            }
        }

        /// Write a new entity.
        ///
        /// # Arguments
        ///   * `model_selector` - the model selector
        ///   * `entity_id` - the id used to identify the record
        ///   * `values` - the field values of the record
        ///   * `layout` - the model layout
        fn write_model_entity(
            ref self: ContractState,
            model_selector: felt252,
            entity_id: felt252,
            values: Span<felt252>,
            layout: Layout
        ) {
            let mut offset = 0;

            match layout {
                Layout::Fixed(layout) => {
                    storage::layout::write_fixed_layout(
                        model_selector, entity_id, values, ref offset, layout
                    );
                },
                Layout::Struct(layout) => {
                    storage::layout::write_struct_layout(
                        model_selector, entity_id, values, ref offset, layout
                    );
                },
                _ => { panic!("Unexpected layout type for a model."); }
            };
        }

        /// Delete an entity.
        ///
        /// # Arguments
        ///   * `model_selector` - the model selector
        ///   * `entity_id` - the ID of the entity to remove.
        ///   * `layout` - the model layout
        fn delete_model_entity(
            ref self: ContractState, model_selector: felt252, entity_id: felt252, layout: Layout
        ) {
            match layout {
                Layout::Fixed(layout) => {
                    storage::layout::delete_fixed_layout(model_selector, entity_id, layout);
                },
                Layout::Struct(layout) => {
                    storage::layout::delete_struct_layout(model_selector, entity_id, layout);
                },
                _ => { panic!("Unexpected layout type for a model."); }
            };
        }

        /// Read an entity.
        ///
        /// # Arguments
        ///   * `model_selector` - the model selector
        ///   * `entity_id` - the ID of the entity to read.
        ///   * `layout` - the model layout
        fn read_model_entity(
            self: @ContractState, model_selector: felt252, entity_id: felt252, layout: Layout
        ) -> Span<felt252> {
            let mut read_data = ArrayTrait::<felt252>::new();

            match layout {
                Layout::Fixed(layout) => {
                    storage::layout::read_fixed_layout(
                        model_selector, entity_id, ref read_data, layout
                    );
                },
                Layout::Struct(layout) => {
                    storage::layout::read_struct_layout(
                        model_selector, entity_id, ref read_data, layout
                    );
                },
                _ => { panic!("Unexpected layout type for a model."); }
            };

            read_data.span()
        }

        /// Read a model member value.
        ///
        /// # Arguments
        ///   * `model_selector` - the model selector
        ///   * `entity_id` - the ID of the entity for which to read a member.
        ///   * `member_id` - the selector of the model member to read.
        ///   * `layout` - the model layout
        fn read_model_member(
            self: @ContractState,
            model_selector: felt252,
            entity_id: felt252,
            member_id: felt252,
            layout: Layout
        ) -> Span<felt252> {
            let mut read_data = ArrayTrait::<felt252>::new();
            storage::layout::read_layout(
                model_selector,
                dojo::utils::combine_key(entity_id, member_id),
                ref read_data,
                layout
            );

            read_data.span()
        }

        /// Write a model member value.
        ///
        /// # Arguments
        ///   * `model_selector` - the model selector
        ///   * `entity_id` - the ID of the entity for which to write a member.
        ///   * `member_id` - the selector of the model member to write.
        ///   * `values` - the new member value.
        ///   * `layout` - the model layout
        fn write_model_member(
            self: @ContractState,
            model_selector: felt252,
            entity_id: felt252,
            member_id: felt252,
            values: Span<felt252>,
            layout: Layout
        ) {
            let mut offset = 0;
            storage::layout::write_layout(
                model_selector,
                dojo::utils::combine_key(entity_id, member_id),
                values,
                ref offset,
                layout
            )
        }
    }
}
