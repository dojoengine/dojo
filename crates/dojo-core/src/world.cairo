use starknet::{ContractAddress, ClassHash, storage_access::StorageBaseAddress, SyscallResult};
use core::traits::{Into, TryInto};
use core::option::OptionTrait;
use dojo::resource_metadata::ResourceMetadata;

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub enum ModelIndex {
    Keys: Span<felt252>,
    Id: felt252,
    // (entity_id, member_id)
    MemberId: (felt252, felt252)
}

#[starknet::interface]
pub trait IWorld<T> {
    fn metadata(self: @T, resource_id: felt252) -> ResourceMetadata;
    fn set_metadata(ref self: T, metadata: ResourceMetadata);
    fn model(self: @T, selector: felt252) -> (ClassHash, ContractAddress);
    fn contract(self: @T, selector: felt252) -> (ClassHash, ContractAddress);
    fn register_model(ref self: T, class_hash: ClassHash);
    fn register_namespace(ref self: T, namespace: ByteArray);
    fn deploy_contract(
        ref self: T, salt: felt252, class_hash: ClassHash, init_calldata: Span<felt252>
    ) -> ContractAddress;
    fn upgrade_contract(ref self: T, selector: felt252, class_hash: ClassHash) -> ClassHash;
    fn uuid(ref self: T) -> usize;
    fn emit(self: @T, keys: Array<felt252>, values: Span<felt252>);

    fn entity(
        self: @T,
        model_selector: felt252,
        index: ModelIndex,
        layout: dojo::database::introspect::Layout
    ) -> Span<felt252>;
    fn set_entity(
        ref self: T,
        model_selector: felt252,
        index: ModelIndex,
        values: Span<felt252>,
        layout: dojo::database::introspect::Layout
    );
    fn delete_entity(
        ref self: T,
        model_selector: felt252,
        index: ModelIndex,
        layout: dojo::database::introspect::Layout
    );

    fn base(self: @T) -> ClassHash;

    /// In Dojo, there are 2 levels of authorization: `owner` and `writer`.
    /// Only accounts can own a resource while any contract can write to a resource,
    /// as soon as it has granted the write access from an owner of the resource.
    fn is_owner(self: @T, address: ContractAddress, resource: felt252) -> bool;
    fn grant_owner(ref self: T, address: ContractAddress, resource: felt252);
    fn revoke_owner(ref self: T, address: ContractAddress, resource: felt252);

    fn is_writer(self: @T, resource: felt252, contract: ContractAddress) -> bool;
    fn grant_writer(ref self: T, resource: felt252, contract: ContractAddress);
    fn revoke_writer(ref self: T, resource: felt252, contract: ContractAddress);

    fn can_write_resource(self: @T, resource_id: felt252, contract: ContractAddress) -> bool;
    fn can_write_model(self: @T, selector: felt252, contract: ContractAddress) -> bool;
    fn can_write_contract(self: @T, selector: felt252, contract: ContractAddress) -> bool;
    fn can_write_namespace(self: @T, selector: felt252, contract: ContractAddress) -> bool;
}

#[starknet::interface]
pub trait IUpgradeableWorld<T> {
    fn upgrade(ref self: T, new_class_hash: ClassHash);
}

#[starknet::interface]
pub trait IWorldProvider<T> {
    fn world(self: @T) -> IWorldDispatcher;
}

pub mod Errors {
    pub const METADATA_DESER: felt252 = 'metadata deser error';
    pub const NOT_OWNER: felt252 = 'not owner';
    pub const NOT_OWNER_WRITER: felt252 = 'not owner or writer';
    pub const NO_WRITE_ACCESS: felt252 = 'no write access';
    pub const NO_MODEL_WRITE_ACCESS: felt252 = 'no model write access';
    pub const NO_NAMESPACE_WRITE_ACCESS: felt252 = 'no namespace write access';
    pub const NAMESPACE_NOT_REGISTERED: felt252 = 'namespace not registered';
    pub const NOT_REGISTERED: felt252 = 'resource not registered';
    pub const INVALID_MODEL_NAME: felt252 = 'invalid model name';
    pub const INVALID_NAMESPACE_NAME: felt252 = 'invalid namespace name';
    pub const INVALID_RESOURCE_SELECTOR: felt252 = 'invalid resource selector';
    pub const OWNER_ONLY_UPGRADE: felt252 = 'only owner can upgrade';
    pub const OWNER_ONLY_UPDATE: felt252 = 'only owner can update';
    pub const NAMESPACE_ALREADY_REGISTERED: felt252 = 'namespace already registered';
    pub const DELETE_ENTITY_MEMBER: felt252 = 'cannot delete entity member';
    pub const UNEXPECTED_ERROR: felt252 = 'unexpected error';
}

#[starknet::contract]
pub mod world {
    use dojo::config::interface::IConfig;
    use core::to_byte_array::FormatAsByteArray;
    use core::traits::TryInto;
    use core::array::{ArrayTrait, SpanTrait};
    use core::traits::Into;
    use core::option::OptionTrait;
    use core::box::BoxTrait;
    use starknet::event::EventEmitter;
    use core::serde::Serde;
    use core::hash::{HashStateExTrait, HashStateTrait};
    use core::pedersen::PedersenTrait;
    use starknet::{
        contract_address_const, get_caller_address, get_contract_address, get_tx_info, ClassHash,
        ContractAddress, syscalls::{deploy_syscall, emit_event_syscall, replace_class_syscall},
        SyscallResult, SyscallResultTrait, storage::Map,
    };
    use core::num::traits::Zero;

    use dojo::database;
    use dojo::database::introspect::{Introspect, Layout, FieldLayout};
    use dojo::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
    use dojo::contract::{IContractDispatcher, IContractDispatcherTrait};
    use dojo::config::component::Config;
    use dojo::model::{Model, IModelDispatcher, IModelDispatcherTrait};
    use dojo::interfaces::{
        IUpgradeableState, IFactRegistryDispatcher, IFactRegistryDispatcherTrait, StorageUpdate,
        ProgramOutput
    };
    use dojo::world::{IWorldDispatcher, IWorld, IUpgradeableWorld};
    use dojo::resource_metadata;
    use dojo::resource_metadata::{ResourceMetadata, ResourceMetadataTrait};
    use dojo::utils::entity_id_from_keys;

    use super::{Errors, ModelIndex};

    const WORLD: felt252 = 0;

    // the minimum internal size of an empty ByteArray
    const MIN_BYTE_ARRAY_SIZE: u32 = 3;

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
        WorldUpgraded: WorldUpgraded,
        MetadataUpdate: MetadataUpdate,
        NamespaceRegistered: NamespaceRegistered,
        ModelRegistered: ModelRegistered,
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

    #[derive(Drop, starknet::Event)]
    pub struct ContractDeployed {
        pub salt: felt252,
        pub class_hash: ClassHash,
        pub address: ContractAddress,
        pub namespace: ByteArray,
        pub name: ByteArray
    }

    #[derive(Drop, starknet::Event)]
    pub struct ContractUpgraded {
        pub class_hash: ClassHash,
        pub address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    pub struct MetadataUpdate {
        pub resource: felt252,
        pub uri: ByteArray
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct NamespaceRegistered {
        pub namespace: ByteArray,
        pub hash: felt252
    }

    #[derive(Drop, starknet::Event)]
    pub struct ModelRegistered {
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
        models_count: usize,
        resources: Map::<felt252, ResourceData>,
        owners: Map::<(felt252, ContractAddress), bool>,
        writers: Map::<(felt252, ContractAddress), bool>,
        #[substorage(v0)]
        config: Config::Storage,
        initialized_contract: Map::<felt252, bool>,
    }

    #[derive(Drop, starknet::Store, Default, Debug)]
    pub enum ResourceData {
        Model: (ClassHash, ContractAddress),
        Contract: (ClassHash, ContractAddress),
        Namespace,
        World,
        #[default]
        None,
    }

    #[generate_trait]
    impl ResourceDataIsNoneImpl of ResourceDataIsNoneTrait {
        fn is_none(self: @ResourceData) -> bool {
            match self {
                ResourceData::None => true,
                _ => false
            }
        }
    }

    #[constructor]
    fn constructor(ref self: ContractState, contract_base: ClassHash) {
        let creator = starknet::get_tx_info().unbox().account_contract_address;
        self.contract_base.write(contract_base);

        self.resources.write(WORLD, ResourceData::World);
        self
            .resources
            .write(
                dojo::model::Model::<ResourceMetadata>::selector(),
                ResourceData::Model(
                    (resource_metadata::initial_class_hash(), resource_metadata::initial_address())
                )
            );
        self.owners.write((WORLD, creator), true);

        let dojo_namespace_hash = dojo::utils::hash(@"__DOJO__");

        self.resources.write(dojo_namespace_hash, ResourceData::Namespace);
        self.owners.write((dojo_namespace_hash, creator), true);

        self.config.initializer(creator);

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
            let mut values = self
                ._read_model_entity(
                    dojo::model::Model::<ResourceMetadata>::selector(),
                    entity_id_from_keys(array![resource_id].span()),
                    dojo::model::Model::<ResourceMetadata>::layout()
                );

            ResourceMetadataTrait::from_values(resource_id, ref values)
        }

        /// Sets the metadata of the resource.
        ///
        /// # Arguments
        ///
        /// `metadata` - The metadata content for the resource.
        fn set_metadata(ref self: ContractState, metadata: ResourceMetadata) {
            assert(
                self.can_write_resource(metadata.resource_id, get_caller_address()),
                Errors::NO_WRITE_ACCESS
            );

            self
                ._write_model_entity(
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
        /// Note that this resource must have been registered to the world first.
        /// 
        /// # Arguments
        ///
        /// * `address` - The contract address.
        /// * `resource` - The resource.
        fn grant_owner(ref self: ContractState, address: ContractAddress, resource: felt252) {
            assert(!self.resources.read(resource).is_none(), Errors::NOT_REGISTERED);
            assert(self.is_account_owner(resource), Errors::NOT_OWNER);

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
        /// * `address` - The contract address.
        /// * `resource` - The resource.
        fn revoke_owner(ref self: ContractState, address: ContractAddress, resource: felt252) {
            assert(!self.resources.read(resource).is_none(), Errors::NOT_REGISTERED);
            assert(self.is_account_owner(resource), Errors::NOT_OWNER);

            self.owners.write((resource, address), false);

            EventEmitter::emit(ref self, OwnerUpdated { address, resource, value: false });
        }

        /// Checks if the provided contract is a writer of the resource.
        /// 
        /// Note: that this function just indicates if a contract has the `writer` role for the resource,
        /// without applying any specific rule. For example, for a model, the write access right
        /// to the model namespace is not checked. 
        /// It does not even check if the contract is an owner of the resource.
        /// Please use more high-level functions such `can_write_model` for that.
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
            assert(!self.resources.read(resource).is_none(), Errors::NOT_REGISTERED);
            assert(self.is_account_owner(resource), Errors::NOT_OWNER);

            self.writers.write((resource, contract), true);

            EventEmitter::emit(ref self, WriterUpdated { resource, contract, value: true });
        }

        /// Revokes writer permission to the contract for the model.
        /// Can only be called by an existing model writer, owner or the world admin.
        ///
        /// Note that this resource must have been registered to the world first.
        ///
        /// # Arguments
        ///
        /// * `model` - The name of the model.
        /// * `contract` - The name of the contract.
        fn revoke_writer(ref self: ContractState, resource: felt252, contract: ContractAddress) {
            assert(!self.resources.read(resource).is_none(), Errors::NOT_REGISTERED);

            let caller = get_caller_address();
            assert(
                self.is_writer(resource, caller) || self.is_account_owner(resource),
                Errors::NOT_OWNER_WRITER
            );
            self.writers.write((resource, contract), false);

            EventEmitter::emit(ref self, WriterUpdated { resource, contract, value: false });
        }

        /// Checks if the provided contract can write to the resource.
        /// 
        /// Note: Contrary to `is_writer`, this function checks resource specific rules.
        /// For example, for a model, it checks if the contract is a write/owner of the resource,
        /// OR a write/owner of the namespace.
        ///
        /// # Arguments
        ///
        /// * `resource_id` - The resource IUpgradeableDispatcher.
        /// * `contract` - The name of the contract.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the contract can write to the resource, false otherwise
        fn can_write_resource(
            self: @ContractState, resource_id: felt252, contract: ContractAddress
        ) -> bool {
            match self.resources.read(resource_id) {
                ResourceData::Model((_, model_address)) => self
                    ._check_model_write_access(resource_id, model_address, contract),
                ResourceData::Contract((_, contract_address)) => self
                    ._check_contract_write_access(resource_id, contract_address, contract),
                ResourceData::Namespace => self._check_basic_write_access(resource_id, contract),
                ResourceData::World => self._check_basic_write_access(WORLD, contract),
                ResourceData::None => core::panic_with_felt252(Errors::INVALID_RESOURCE_SELECTOR)
            }
        }

        /// Checks if the provided contract can write to the model.
        /// It panics if the resource selector is not a model.
        /// 
        /// Note: Contrary to `is_writer`, this function checks if the contract is a write/owner of the model,
        /// OR a write/owner of the namespace.
        ///
        /// # Arguments
        ///
        /// * `selector` - The model selector.
        /// * `contract` - The name of the contract.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the contract can write to the model, false otherwise
        fn can_write_model(
            self: @ContractState, selector: felt252, contract: ContractAddress
        ) -> bool {
            match self.resources.read(selector) {
                ResourceData::Model((_, model_address)) => self
                    ._check_model_write_access(selector, model_address, contract),
                _ => core::panic_with_felt252(Errors::INVALID_RESOURCE_SELECTOR)
            }
        }

        /// Checks if the provided contract can write to the contract.
        /// It panics if the resource selector is not a contract.
        /// 
        /// Note: Contrary to `is_writer`, this function checks if the contract is a write/owner of the contract,
        /// OR a write/owner of the namespace.
        ///
        /// # Arguments
        ///
        /// * `selector` - The contract selector.
        /// * `contract` - The name of the contract.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the contract can write to the model, false otherwise
        fn can_write_contract(
            self: @ContractState, selector: felt252, contract: ContractAddress
        ) -> bool {
            match self.resources.read(selector) {
                ResourceData::Contract((_, contract_address)) => self
                    ._check_contract_write_access(selector, contract_address, contract),
                _ => core::panic_with_felt252(Errors::INVALID_RESOURCE_SELECTOR)
            }
        }

        /// Checks if the provided contract can write to the namespace.
        /// It panics if the resource selector is not a namespace.
        /// 
        /// Note: Contrary to `is_writer`, this function also checks if the caller account is
        /// the owner of the namespace.
        ///
        /// # Arguments
        ///
        /// * `selector` - The namespace selector.
        /// * `contract` - The name of the contract.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the contract can write to the namespace, false otherwise
        fn can_write_namespace(
            self: @ContractState, selector: felt252, contract: ContractAddress
        ) -> bool {
            match self.resources.read(selector) {
                ResourceData::Namespace => self._check_basic_write_access(selector, contract),
                _ => core::panic_with_felt252(Errors::INVALID_RESOURCE_SELECTOR)
            }
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
            let (address, name, selector, namespace, namespace_hash) =
                dojo::model::deploy_and_get_metadata(
                salt.into(), class_hash
            )
                .unwrap_syscall();
            self.models_count.write(salt + 1);

            let (mut prev_class_hash, mut prev_address) = (
                core::num::traits::Zero::<ClassHash>::zero(),
                core::num::traits::Zero::<ContractAddress>::zero(),
            );

            assert(self._is_namespace_registered(namespace_hash), Errors::NAMESPACE_NOT_REGISTERED);
            assert(
                self.can_write_namespace(namespace_hash, get_caller_address()),
                Errors::NO_NAMESPACE_WRITE_ACCESS
            );

            if selector.is_zero() {
                core::panic_with_felt252(Errors::INVALID_MODEL_NAME);
            }

            match self.resources.read(selector) {
                // If model is already registered, validate permission to update.
                ResourceData::Model((
                    model_hash, model_address
                )) => {
                    assert(self.is_account_owner(selector), Errors::OWNER_ONLY_UPDATE);
                    prev_class_hash = model_hash;
                    prev_address = model_address;
                },
                // new model
                ResourceData::None => { self.owners.write((selector, caller), true); },
                // Avoids a model name to conflict with already registered resource,
                // which can cause ACL issue with current ACL implementation.
                _ => core::panic_with_felt252(Errors::INVALID_MODEL_NAME)
            };

            self.resources.write(selector, ResourceData::Model((class_hash, address)));
            EventEmitter::emit(
                ref self,
                ModelRegistered {
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
            let caller_account = self._get_account_address();

            let hash = dojo::utils::hash(@namespace);

            match self.resources.read(hash) {
                ResourceData::Namespace => {
                    if !self.is_account_owner(hash) {
                        core::panic_with_felt252(Errors::NAMESPACE_ALREADY_REGISTERED);
                    }
                },
                ResourceData::None => {
                    self.resources.write(hash, ResourceData::Namespace);
                    self.owners.write((hash, caller_account), true);

                    EventEmitter::emit(ref self, NamespaceRegistered { namespace, hash });
                },
                _ => { core::panic_with_felt252(Errors::INVALID_NAMESPACE_NAME); }
            };
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
            match self.resources.read(selector) {
                ResourceData::Model(m) => m,
                _ => core::panic_with_felt252(Errors::INVALID_RESOURCE_SELECTOR)
            }
        }

        fn contract(self: @ContractState, selector: felt252) -> (ClassHash, ContractAddress) {
            match self.resources.read(selector) {
                ResourceData::Contract(c) => c,
                _ => core::panic_with_felt252(Errors::INVALID_RESOURCE_SELECTOR)
            }
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

            // namespace checking
            let dispatcher = IContractDispatcher { contract_address };
            let namespace = dispatcher.namespace();
            let name = dispatcher.contract_name();
            let namespace_hash = dispatcher.namespace_hash();
            assert(self._is_namespace_registered(namespace_hash), Errors::NAMESPACE_NOT_REGISTERED);
            assert(
                self.can_write_namespace(namespace_hash, get_caller_address()),
                Errors::NO_NAMESPACE_WRITE_ACCESS
            );

            let selector = dispatcher.selector();

            if self.initialized_contract.read(selector) {
                panic!("Contract has already been initialized");
            } else {
                starknet::syscalls::call_contract_syscall(
                    contract_address, DOJO_INIT_SELECTOR, init_calldata
                )
                    .unwrap_syscall();
                self.initialized_contract.write(selector, true);
            }

            self.owners.write((selector, get_caller_address()), true);

            self.resources.write(selector, ResourceData::Contract((class_hash, contract_address)));

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
            assert(self.is_account_owner(selector), Errors::NOT_OWNER);
            let (_, contract_address) = self.contract(selector);
            IUpgradeableDispatcher { contract_address }.upgrade(class_hash);
            EventEmitter::emit(
                ref self, ContractUpgraded { class_hash, address: contract_address }
            );
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
            self: @ContractState,
            model_selector: felt252,
            index: ModelIndex,
            layout: dojo::database::introspect::Layout
        ) -> Span<felt252> {
            match index {
                ModelIndex::Keys(keys) => {
                    let entity_id = entity_id_from_keys(keys);
                    self._read_model_entity(model_selector, entity_id, layout)
                },
                ModelIndex::Id(entity_id) => {
                    self._read_model_entity(model_selector, entity_id, layout)
                },
                ModelIndex::MemberId((
                    entity_id, member_id
                )) => { self._read_model_member(model_selector, entity_id, member_id, layout) }
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
            layout: dojo::database::introspect::Layout
        ) {
            assert(
                self.can_write_model(model_selector, get_caller_address()),
                Errors::NO_MODEL_WRITE_ACCESS
            );

            match index {
                ModelIndex::Keys(keys) => {
                    let entity_id = entity_id_from_keys(keys);
                    self._write_model_entity(model_selector, entity_id, values, layout);
                    EventEmitter::emit(
                        ref self, StoreSetRecord { table: model_selector, keys, values }
                    );
                },
                ModelIndex::Id(entity_id) => {
                    self._write_model_entity(model_selector, entity_id, values, layout);
                    EventEmitter::emit(
                        ref self, StoreUpdateRecord { table: model_selector, entity_id, values }
                    );
                },
                ModelIndex::MemberId((
                    entity_id, member_selector
                )) => {
                    self
                        ._write_model_member(
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

        /// Deletes a record/entity of a model..
        /// Deleting is setting all the values to 0 in the given layout.
        ///
        /// # Arguments
        ///
        /// * `model_selector` - The selector of the model to be deleted.
        /// * `index` - The index of the record/entity to delete.
        /// * `layout` - The memory layout of the model.
        fn delete_entity(
            ref self: ContractState,
            model_selector: felt252,
            index: ModelIndex,
            layout: dojo::database::introspect::Layout
        ) {
            assert(
                self.can_write_model(model_selector, get_caller_address()),
                Errors::NO_MODEL_WRITE_ACCESS
            );

            match index {
                ModelIndex::Keys(keys) => {
                    let entity_id = entity_id_from_keys(keys);
                    self._delete_model_entity(model_selector, entity_id, layout);
                    EventEmitter::emit(
                        ref self, StoreDelRecord { table: model_selector, entity_id }
                    );
                },
                ModelIndex::Id(entity_id) => {
                    self._delete_model_entity(model_selector, entity_id, layout);
                    EventEmitter::emit(
                        ref self, StoreDelRecord { table: model_selector, entity_id }
                    );
                },
                ModelIndex::MemberId(_) => {
                    core::panic_with_felt252(Errors::DELETE_ENTITY_MEMBER);
                }
            }
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
            assert(self.is_account_world_owner(), Errors::OWNER_ONLY_UPGRADE);

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
    impl Self of SelfTrait {
        #[inline(always)]
        fn _get_account_address(self: @ContractState) -> ContractAddress {
            get_tx_info().unbox().account_contract_address
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
        #[inline(always)]
        fn is_account_owner(self: @ContractState, resource: felt252) -> bool {
            IWorld::is_owner(self, self._get_account_address(), resource)
                || self.is_account_world_owner()
        }

        /// Verifies if the calling account has write access to the resource.
        ///
        /// # Arguments
        ///
        /// * `resource` - The selector of the resource being verified.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the calling account has write access to the resource,
        ///            false otherwise.
        #[inline(always)]
        fn is_account_writer(self: @ContractState, resource: felt252) -> bool {
            IWorld::is_writer(self, resource, self._get_account_address())
        }

        /// Verifies if the calling account is the world owner.
        ///
        /// # Returns
        ///
        /// * `bool` - True if the calling account is the world owner, false otherwise.
        #[inline(always)]
        fn is_account_world_owner(self: @ContractState) -> bool {
            IWorld::is_owner(self, self._get_account_address(), WORLD)
        }

        /// Indicates if the provided namespace is already registered
        #[inline(always)]
        fn _is_namespace_registered(self: @ContractState, namespace_hash: felt252) -> bool {
            match self.resources.read(namespace_hash) {
                ResourceData::Namespace => true,
                _ => false
            }
        }

        /// Check model write access.
        /// That means, check if:
        /// - the calling contract has the writer role for the model OR,
        /// - the calling account has the owner and/or writer role for the model OR,
        /// - the calling contract has the writer role for the model namespace OR
        /// - the calling account has the owner and/or writer role for the model namespace.
        /// 
        /// # Arguments
        ///  * `model_selector` - the model selector to check.
        ///  * `model_address` - the model contract address.
        ///  * `contract` - the calling contract.
        /// 
        /// # Returns
        ///  `true` if the write access is allowed, false otherwise.
        /// 
        fn _check_model_write_access(
            self: @ContractState,
            model_selector: felt252,
            model_address: ContractAddress,
            contract: ContractAddress
        ) -> bool {
            if !self.is_writer(model_selector, contract)
                && !self.is_account_owner(model_selector)
                && !self.is_account_writer(model_selector) {
                let model = IModelDispatcher { contract_address: model_address };
                self._check_basic_write_access(model.namespace_hash(), contract)
            } else {
                true
            }
        }

        /// Check contract write access.
        /// That means, check if:
        /// - the calling contract has the writer role for the contract OR,
        /// - the calling account has the owner and/or writer role for the contract OR,
        /// - the calling contract has the writer role for the contract namespace OR
        /// - the calling account has the owner and/or writer role for the model namespace.
        /// 
        /// # Arguments
        ///  * `contract_selector` - the contract selector to check.
        ///  * `contract_address` - the contract contract address.
        ///  * `contract` - the calling contract.
        /// 
        /// # Returns
        ///  `true` if the write access is allowed, false otherwise.
        /// 
        fn _check_contract_write_access(
            self: @ContractState,
            contract_selector: felt252,
            contract_address: ContractAddress,
            contract: ContractAddress
        ) -> bool {
            if !self.is_writer(contract_selector, contract)
                && !self.is_account_owner(contract_selector)
                && !self.is_account_writer(contract_selector) {
                let dispatcher = IContractDispatcher { contract_address };
                self._check_basic_write_access(dispatcher.namespace_hash(), contract)
            } else {
                true
            }
        }

        /// Check basic resource write access.
        /// That means, check if:
        /// - the calling contract has the writer role for the resource OR,
        /// - the calling account has the owner and/or writer role for the resource.
        /// 
        /// # Arguments
        ///  * `resource_id` - the resource selector to check.
        ///  * `contract` - the calling contract.
        /// 
        /// # Returns
        ///  `true` if the write access is allowed, false otherwise.
        /// 
        fn _check_basic_write_access(
            self: @ContractState, resource_id: felt252, contract: ContractAddress
        ) -> bool {
            self.is_writer(resource_id, contract)
                || self.is_account_owner(resource_id)
                || self.is_account_writer(resource_id)
        }

        /// Write a new entity.
        ///
        /// # Arguments
        ///   * `model_selector` - the model selector
        ///   * `entity_id` - the id used to identify the record
        ///   * `values` - the field values of the record
        ///   * `layout` - the model layout
        fn _write_model_entity(
            ref self: ContractState,
            model_selector: felt252,
            entity_id: felt252,
            values: Span<felt252>,
            layout: dojo::database::introspect::Layout
        ) {
            let mut offset = 0;

            match layout {
                Layout::Fixed(layout) => {
                    Self::_write_fixed_layout(
                        model_selector, entity_id, values, ref offset, layout
                    );
                },
                Layout::Struct(layout) => {
                    Self::_write_struct_layout(
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
        fn _delete_model_entity(
            ref self: ContractState,
            model_selector: felt252,
            entity_id: felt252,
            layout: dojo::database::introspect::Layout
        ) {
            match layout {
                Layout::Fixed(layout) => {
                    Self::_delete_fixed_layout(model_selector, entity_id, layout);
                },
                Layout::Struct(layout) => {
                    Self::_delete_struct_layout(model_selector, entity_id, layout);
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
        fn _read_model_entity(
            self: @ContractState,
            model_selector: felt252,
            entity_id: felt252,
            layout: dojo::database::introspect::Layout
        ) -> Span<felt252> {
            let mut read_data = ArrayTrait::<felt252>::new();

            match layout {
                Layout::Fixed(layout) => {
                    Self::_read_fixed_layout(model_selector, entity_id, ref read_data, layout);
                },
                Layout::Struct(layout) => {
                    Self::_read_struct_layout(model_selector, entity_id, ref read_data, layout);
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
        fn _read_model_member(
            self: @ContractState,
            model_selector: felt252,
            entity_id: felt252,
            member_id: felt252,
            layout: dojo::database::introspect::Layout
        ) -> Span<felt252> {
            let mut read_data = ArrayTrait::<felt252>::new();
            Self::_read_layout(
                model_selector, Self::_combine_key(entity_id, member_id), ref read_data, layout
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
        fn _write_model_member(
            self: @ContractState,
            model_selector: felt252,
            entity_id: felt252,
            member_id: felt252,
            values: Span<felt252>,
            layout: dojo::database::introspect::Layout
        ) {
            let mut offset = 0;
            Self::_write_layout(
                model_selector, Self::_combine_key(entity_id, member_id), values, ref offset, layout
            )
        }

        /// Combine parent and child keys to build one full key.
        fn _combine_key(parent_key: felt252, child_key: felt252) -> felt252 {
            core::poseidon::poseidon_hash_span(array![parent_key, child_key].span())
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
                let key = Self::_combine_key(key, i.into());

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
                let field_key = Self::_combine_key(key, field_layout.selector);

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
                let key = Self::_combine_key(key, i.into());

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

            // and write it
            database::set(model, key, values, offset, array![251].span());
            offset += 1;

            // find the corresponding layout and then write the full variant
            let variant_data_key = Self::_combine_key(key, variant);

            match Self::_find_variant_layout(variant, variant_layouts) {
                Option::Some(layout) => Self::_write_layout(
                    model, variant_data_key, values, ref offset, layout
                ),
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
                let key = Self::_combine_key(key, field_layout.selector);

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
                let key = Self::_combine_key(key, i.into());

                Self::_delete_layout(model, key, field_layout);

                i += 1;
            }
        }

        fn _delete_enum_layout(model: felt252, key: felt252, variant_layouts: Span<FieldLayout>) {
            // read the variant value
            let res = database::get(model, key, array![251].span());
            assert(res.len() == 1, 'internal database error');

            let variant = *res.at(0);
            assert(variant.into() < 256_u256, 'invalid variant value');

            // reset the variant value
            database::delete(model, key, array![251].span());

            // find the corresponding layout and the delete the full variant
            let variant_data_key = Self::_combine_key(key, variant);

            match Self::_find_variant_layout(variant, variant_layouts) {
                Option::Some(layout) => Self::_delete_layout(model, variant_data_key, layout),
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

                let field_key = Self::_combine_key(key, i.into());
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
                let field_key = Self::_combine_key(key, field_layout.selector);

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
                let field_key = Self::_combine_key(key, i.into());
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
            // read the variant value first
            let res = database::get(model, key, array![8].span());
            assert(res.len() == 1, 'internal database error');

            let variant = *res.at(0);
            assert(variant.into() < 256_u256, 'invalid variant value');

            read_data.append(variant);

            // find the corresponding layout and the read the variant data
            let variant_data_key = Self::_combine_key(key, variant);

            match Self::_find_variant_layout(variant, variant_layouts) {
                Option::Some(layout) => Self::_read_layout(
                    model, variant_data_key, ref read_data, layout
                ),
                Option::None => panic!("Unable to find the variant layout")
            };
        }
    }
}
