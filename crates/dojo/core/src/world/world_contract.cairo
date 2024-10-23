use core::fmt::{Display, Formatter, Error};

#[derive(Copy, Drop, PartialEq)]
pub enum Permission {
    Writer,
    Owner,
}

impl PermissionDisplay of Display<Permission> {
    fn fmt(self: @Permission, ref f: Formatter) -> Result<(), Error> {
        let str = match self {
            Permission::Writer => @"WRITER",
            Permission::Owner => @"OWNER",
        };
        f.buffer.append(str);
        Result::Ok(())
    }
}

#[starknet::contract]
pub mod world {
    use core::array::ArrayTrait;
    use core::box::BoxTrait;
    use core::num::traits::Zero;
    use core::traits::Into;
    use core::panic_with_felt252;
    use core::panics::panic_with_byte_array;

    use starknet::{
        get_caller_address, get_tx_info, ClassHash, ContractAddress,
        syscalls::{deploy_syscall, replace_class_syscall}, SyscallResultTrait, storage::Map,
    };
    pub use starknet::storage::{
        StorageMapReadAccess, StorageMapWriteAccess, StoragePointerReadAccess,
        StoragePointerWriteAccess
    };

    use dojo::world::errors;
    use dojo::contract::components::upgradeable::{
        IUpgradeableDispatcher, IUpgradeableDispatcherTrait
    };
    use dojo::contract::{IContractDispatcher, IContractDispatcherTrait};
    use dojo::meta::Layout;
    use dojo::model::{
        Model, ResourceMetadata, metadata, ModelIndex, IModelDispatcher, IModelDispatcherTrait
    };
    use dojo::event::{IEventDispatcher, IEventDispatcherTrait};
    use dojo::storage;
    use dojo::utils::{entity_id_from_keys, bytearray_hash, selector_from_namespace_and_name};
    use dojo::world::{IWorld, IUpgradeableWorld, Resource, ResourceIsNoneTrait};
    use super::Permission;

    pub const WORLD: felt252 = 0;
    pub const DOJO_INIT_SELECTOR: felt252 = selector!("dojo_init");

    #[event]
    #[derive(Drop, starknet::Event)]
    pub enum Event {
        WorldSpawned: WorldSpawned,
        WorldUpgraded: WorldUpgraded,
        NamespaceRegistered: NamespaceRegistered,
        ModelRegistered: ModelRegistered,
        EventRegistered: EventRegistered,
        ContractRegistered: ContractRegistered,
        ModelUpgraded: ModelUpgraded,
        EventUpgraded: EventUpgraded,
        ContractUpgraded: ContractUpgraded,
        ContractInitialized: ContractInitialized,
        EventEmitted: EventEmitted,
        MetadataUpdate: MetadataUpdate,
        StoreSetRecord: StoreSetRecord,
        StoreUpdateRecord: StoreUpdateRecord,
        StoreUpdateMember: StoreUpdateMember,
        StoreDelRecord: StoreDelRecord,
        WriterUpdated: WriterUpdated,
        OwnerUpdated: OwnerUpdated,
    }

    #[derive(Drop, starknet::Event)]
    pub struct WorldSpawned {
        pub creator: ContractAddress,
        pub class_hash: ClassHash,
    }

    #[derive(Drop, starknet::Event)]
    pub struct WorldUpgraded {
        pub class_hash: ClassHash,
    }

    #[derive(Drop, starknet::Event)]
    pub struct ContractRegistered {
        #[key]
        pub selector: felt252,
        pub address: ContractAddress,
        pub class_hash: ClassHash,
        pub salt: felt252,
    }

    #[derive(Drop, starknet::Event)]
    pub struct ContractUpgraded {
        #[key]
        pub selector: felt252,
        pub class_hash: ClassHash,
    }

    #[derive(Drop, starknet::Event)]
    pub struct MetadataUpdate {
        #[key]
        pub resource: felt252,
        pub uri: ByteArray
    }

    #[derive(Drop, starknet::Event)]
    pub struct NamespaceRegistered {
        #[key]
        pub namespace: ByteArray,
        pub hash: felt252
    }

    #[derive(Drop, starknet::Event)]
    pub struct ModelRegistered {
        #[key]
        pub name: ByteArray,
        #[key]
        pub namespace: ByteArray,
        pub class_hash: ClassHash,
        pub address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    pub struct ModelUpgraded {
        #[key]
        pub selector: felt252,
        pub class_hash: ClassHash,
        pub address: ContractAddress,
        pub prev_address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    pub struct EventRegistered {
        #[key]
        pub name: ByteArray,
        #[key]
        pub namespace: ByteArray,
        pub class_hash: ClassHash,
        pub address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    pub struct EventUpgraded {
        #[key]
        pub selector: felt252,
        pub class_hash: ClassHash,
        pub address: ContractAddress,
        pub prev_address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreSetRecord {
        #[key]
        pub table: felt252,
        #[key]
        pub entity_id: felt252,
        pub keys: Span<felt252>,
        pub values: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreUpdateRecord {
        #[key]
        pub table: felt252,
        #[key]
        pub entity_id: felt252,
        pub values: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreUpdateMember {
        #[key]
        pub table: felt252,
        #[key]
        pub entity_id: felt252,
        #[key]
        pub member_selector: felt252,
        pub values: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreDelRecord {
        #[key]
        pub table: felt252,
        #[key]
        pub entity_id: felt252,
    }

    #[derive(Drop, starknet::Event)]
    pub struct WriterUpdated {
        #[key]
        pub resource: felt252,
        #[key]
        pub contract: ContractAddress,
        pub value: bool
    }

    #[derive(Drop, starknet::Event)]
    pub struct OwnerUpdated {
        #[key]
        pub resource: felt252,
        #[key]
        pub contract: ContractAddress,
        pub value: bool,
    }

    #[derive(Drop, starknet::Event)]
    pub struct ContractInitialized {
        #[key]
        pub selector: felt252,
        pub init_calldata: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    pub struct EventEmitted {
        #[key]
        pub event_selector: felt252,
        #[key]
        pub system_address: ContractAddress,
        #[key]
        pub historical: bool,
        pub keys: Span<felt252>,
        pub values: Span<felt252>,
    }

    #[storage]
    struct Storage {
        nonce: usize,
        models_salt: usize,
        events_salt: usize,
        resources: Map::<felt252, Resource>,
        owners: Map::<(felt252, ContractAddress), bool>,
        writers: Map::<(felt252, ContractAddress), bool>,
        initialized_contracts: Map::<felt252, bool>,
    }

    /// Constructor for the world contract.
    ///
    /// # Arguments
    ///
    /// * `world_class_hash` - The class hash of the world contract that is being deployed.
    ///   As currently Starknet doesn't support a syscall to get the class hash of the
    ///   deploying contract, the hash of the world contract has to be provided at spawn time
    ///   This also ensures the world's address is always deterministic since the world class
    ///   hash can change when the world contract is upgraded.
    #[constructor]
    fn constructor(ref self: ContractState, world_class_hash: ClassHash) {
        let creator = starknet::get_tx_info().unbox().account_contract_address;

        let (internal_ns, internal_ns_hash) = self.world_internal_namespace();

        self.resources.write(internal_ns_hash, Resource::Namespace(internal_ns));
        self.owners.write((internal_ns_hash, creator), true);

        self.resources.write(WORLD, Resource::World);
        self.owners.write((WORLD, creator), true);

        // This model doesn't need to have the class hash or the contract address
        // set since they are manually controlled by the world contract.
        self
            .resources
            .write(
                metadata::resource_metadata_selector(internal_ns_hash),
                Resource::Model(
                    (metadata::default_address(), metadata::default_class_hash().into())
                )
            );

        self.emit(WorldSpawned { creator, class_hash: world_class_hash });
    }

    #[cfg(target: "test")]
    #[abi(embed_v0)]
    impl WorldTestImpl of dojo::world::IWorldTest<ContractState> {
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

        fn emit_event_test(
            ref self: ContractState,
            event_selector: felt252,
            keys: Span<felt252>,
            values: Span<felt252>,
            historical: bool
        ) {
            self
                .emit(
                    EventEmitted {
                        event_selector,
                        system_address: get_caller_address(),
                        historical,
                        keys,
                        values
                    }
                );
        }
    }

    #[abi(embed_v0)]
    impl World of IWorld<ContractState> {
        fn metadata(self: @ContractState, resource_selector: felt252) -> ResourceMetadata {
            let (_, internal_ns_hash) = self.world_internal_namespace();

            let mut values = storage::entity_model::read_model_entity(
                metadata::resource_metadata_selector(internal_ns_hash),
                entity_id_from_keys([resource_selector].span()),
                Model::<ResourceMetadata>::layout()
            );

            let mut keys = [resource_selector].span();

            match Model::<ResourceMetadata>::from_values(ref keys, ref values) {
                Option::Some(x) => x,
                Option::None => panic!("Model `ResourceMetadata`: deserialization failed.")
            }
        }

        fn set_metadata(ref self: ContractState, metadata: ResourceMetadata) {
            self.assert_caller_permissions(metadata.resource_id, Permission::Owner);

            let (_, internal_ns_hash) = self.world_internal_namespace();

            storage::entity_model::write_model_entity(
                metadata::resource_metadata_selector(internal_ns_hash),
                metadata.resource_id,
                metadata.values(),
                Model::<ResourceMetadata>::layout()
            );

            self
                .emit(
                    MetadataUpdate { resource: metadata.resource_id, uri: metadata.metadata_uri }
                );
        }

        fn is_owner(self: @ContractState, resource: felt252, address: ContractAddress) -> bool {
            self.owners.read((resource, address))
        }

        fn grant_owner(ref self: ContractState, resource: felt252, address: ContractAddress) {
            if self.resources.read(resource).is_unregistered() {
                panic_with_byte_array(@errors::resource_not_registered(resource));
            }

            self.assert_caller_permissions(resource, Permission::Owner);

            self.owners.write((resource, address), true);

            self.emit(OwnerUpdated { contract: address, resource, value: true });
        }

        fn revoke_owner(ref self: ContractState, resource: felt252, address: ContractAddress) {
            if self.resources.read(resource).is_unregistered() {
                panic_with_byte_array(@errors::resource_not_registered(resource));
            }

            self.assert_caller_permissions(resource, Permission::Owner);

            self.owners.write((resource, address), false);

            self.emit(OwnerUpdated { contract: address, resource, value: false });
        }

        fn is_writer(self: @ContractState, resource: felt252, contract: ContractAddress) -> bool {
            self.writers.read((resource, contract))
        }

        fn grant_writer(ref self: ContractState, resource: felt252, contract: ContractAddress) {
            if self.resources.read(resource).is_unregistered() {
                panic_with_byte_array(@errors::resource_not_registered(resource));
            }

            self.assert_caller_permissions(resource, Permission::Owner);

            self.writers.write((resource, contract), true);

            self.emit(WriterUpdated { resource, contract, value: true });
        }

        fn revoke_writer(ref self: ContractState, resource: felt252, contract: ContractAddress) {
            if self.resources.read(resource).is_unregistered() {
                panic_with_byte_array(@errors::resource_not_registered(resource));
            }

            self.assert_caller_permissions(resource, Permission::Owner);

            self.writers.write((resource, contract), false);

            self.emit(WriterUpdated { resource, contract, value: false });
        }

        fn register_event(ref self: ContractState, namespace: ByteArray, class_hash: ClassHash) {
            let caller = get_caller_address();
            let salt = self.events_salt.read();

            let namespace_hash = bytearray_hash(@namespace);

            let (contract_address, _) = starknet::syscalls::deploy_syscall(
                class_hash, salt.into(), [].span(), false,
            )
                .unwrap_syscall();
            self.events_salt.write(salt + 1);

            let event = IEventDispatcher { contract_address };
            let event_name = event.dojo_name();

            self.assert_name(@event_name);

            let event_selector = selector_from_namespace_and_name(namespace_hash, @event_name);

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_permissions(namespace_hash, Permission::Owner);

            let maybe_existing_event = self.resources.read(event_selector);
            if !maybe_existing_event.is_unregistered() {
                panic_with_byte_array(@errors::event_already_registered(@namespace, @event_name));
            }

            self
                .resources
                .write(event_selector, Resource::Event((contract_address, namespace_hash)));
            self.owners.write((event_selector, caller), true);

            self
                .emit(
                    EventRegistered {
                        name: event_name.clone(),
                        namespace: namespace.clone(),
                        address: contract_address,
                        class_hash
                    }
                );
        }

        fn upgrade_event(ref self: ContractState, namespace: ByteArray, class_hash: ClassHash) {
            let salt = self.events_salt.read();

            let (new_contract_address, _) = starknet::syscalls::deploy_syscall(
                class_hash, salt.into(), [].span(), false,
            )
                .unwrap_syscall();

            self.events_salt.write(salt + 1);

            let namespace_hash = bytearray_hash(@namespace);

            let event = IEventDispatcher { contract_address: new_contract_address };
            let event_name = event.dojo_name();
            let event_selector = selector_from_namespace_and_name(namespace_hash, @event_name);

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_permissions(event_selector, Permission::Owner);

            let mut prev_address = core::num::traits::Zero::<ContractAddress>::zero();

            // If the namespace or name of the event have been changed, the selector
            // will be different, hence not upgradeable.
            match self.resources.read(event_selector) {
                Resource::Event((model_address, _)) => { prev_address = model_address; },
                Resource::Unregistered => {
                    panic_with_byte_array(
                        @errors::resource_not_registered_details(@namespace, @event_name)
                    )
                },
                _ => panic_with_byte_array(
                    @errors::resource_conflict(@format!("{}-{}", @namespace, @event_name), @"event")
                )
            };

            self
                .resources
                .write(event_selector, Resource::Event((new_contract_address, namespace_hash)));

            self
                .emit(
                    EventUpgraded {
                        selector: event_selector,
                        prev_address,
                        address: new_contract_address,
                        class_hash,
                    }
                );
        }

        fn register_model(ref self: ContractState, namespace: ByteArray, class_hash: ClassHash) {
            let caller = get_caller_address();
            let salt = self.models_salt.read();

            let namespace_hash = bytearray_hash(@namespace);

            let (contract_address, _) = starknet::syscalls::deploy_syscall(
                class_hash, salt.into(), [].span(), false,
            )
                .unwrap_syscall();
            self.models_salt.write(salt + 1);

            let model = IModelDispatcher { contract_address };
            let model_name = model.dojo_name();

            self.assert_name(@model_name);

            let model_selector = selector_from_namespace_and_name(namespace_hash, @model_name);

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_permissions(namespace_hash, Permission::Owner);

            let maybe_existing_model = self.resources.read(model_selector);
            if !maybe_existing_model.is_unregistered() {
                panic_with_byte_array(@errors::model_already_registered(@namespace, @model_name));
            }

            self
                .resources
                .write(model_selector, Resource::Model((contract_address, namespace_hash)));
            self.owners.write((model_selector, caller), true);

            self
                .emit(
                    ModelRegistered {
                        name: model_name.clone(),
                        namespace: namespace.clone(),
                        address: contract_address,
                        class_hash
                    }
                );
        }

        fn upgrade_model(ref self: ContractState, namespace: ByteArray, class_hash: ClassHash) {
            let salt = self.models_salt.read();

            let (new_contract_address, _) = starknet::syscalls::deploy_syscall(
                class_hash, salt.into(), [].span(), false,
            )
                .unwrap_syscall();

            self.models_salt.write(salt + 1);

            let namespace_hash = bytearray_hash(@namespace);

            let model = IModelDispatcher { contract_address: new_contract_address };
            let model_name = model.dojo_name();
            let model_selector = selector_from_namespace_and_name(namespace_hash, @model_name);

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_permissions(model_selector, Permission::Owner);

            let mut prev_address = core::num::traits::Zero::<ContractAddress>::zero();

            // If the namespace or name of the model have been changed, the selector
            // will be different, hence detected as not registered as model.
            match self.resources.read(model_selector) {
                Resource::Model((model_address, _)) => { prev_address = model_address; },
                Resource::Unregistered => {
                    panic_with_byte_array(
                        @errors::resource_not_registered_details(@namespace, @model_name)
                    )
                },
                _ => panic_with_byte_array(
                    @errors::resource_conflict(@format!("{}-{}", @namespace, @model_name), @"model")
                )
            };

            // TODO(@remy): check upgradeability with the actual content of the model.
            // Use `prev_address` to get the previous model address and get `Ty` from it.

            self
                .resources
                .write(model_selector, Resource::Model((new_contract_address, namespace_hash)));

            self
                .emit(
                    ModelUpgraded {
                        selector: model_selector,
                        prev_address,
                        address: new_contract_address,
                        class_hash,
                    }
                );
        }

        fn register_namespace(ref self: ContractState, namespace: ByteArray) {
            self.assert_namespace(@namespace);

            let caller = get_caller_address();

            let hash = bytearray_hash(@namespace);

            match self.resources.read(hash) {
                Resource::Namespace => panic_with_byte_array(
                    @errors::namespace_already_registered(@namespace)
                ),
                Resource::Unregistered => {
                    self.resources.write(hash, Resource::Namespace(namespace.clone()));
                    self.owners.write((hash, caller), true);

                    self.emit(NamespaceRegistered { namespace, hash });
                },
                _ => {
                    panic_with_byte_array(@errors::resource_conflict(@namespace, @"namespace"));
                }
            };
        }

        fn register_contract(
            ref self: ContractState, salt: felt252, namespace: ByteArray, class_hash: ClassHash,
        ) -> ContractAddress {
            let caller = get_caller_address();

            let (contract_address, _) = deploy_syscall(class_hash, salt, [].span(), false)
                .unwrap_syscall();

            let namespace_hash = bytearray_hash(@namespace);

            let contract = IContractDispatcher { contract_address };
            let contract_name = contract.dojo_name();
            let contract_selector = selector_from_namespace_and_name(
                namespace_hash, @contract_name
            );

            self.assert_name(@contract_name);

            let maybe_existing_contract = self.resources.read(contract_selector);
            if !maybe_existing_contract.is_unregistered() {
                panic_with_byte_array(
                    @errors::contract_already_registered(@namespace, @contract_name)
                );
            }

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_permissions(namespace_hash, Permission::Owner);

            self.owners.write((contract_selector, caller), true);
            self
                .resources
                .write(contract_selector, Resource::Contract((contract_address, namespace_hash)));

            self
                .emit(
                    ContractRegistered {
                        salt, class_hash, address: contract_address, selector: contract_selector,
                    }
                );

            contract_address
        }

        fn upgrade_contract(
            ref self: ContractState, namespace: ByteArray, class_hash: ClassHash
        ) -> ClassHash {
            let (new_contract_address, _) = deploy_syscall(
                class_hash, starknet::get_tx_info().unbox().transaction_hash, [].span(), false
            )
                .unwrap_syscall();

            let namespace_hash = bytearray_hash(@namespace);

            let contract = IContractDispatcher { contract_address: new_contract_address };
            let contract_name = contract.dojo_name();
            let contract_selector = selector_from_namespace_and_name(
                namespace_hash, @contract_name
            );

            // If namespace and name are the same, the contract is already registered and we
            // can upgrade it.
            match self.resources.read(contract_selector) {
                Resource::Contract((
                    contract_address, _
                )) => {
                    self.assert_caller_permissions(contract_selector, Permission::Owner);

                    IUpgradeableDispatcher { contract_address }.upgrade(class_hash);
                    self.emit(ContractUpgraded { class_hash, selector: contract_selector });

                    class_hash
                },
                Resource::Unregistered => {
                    panic_with_byte_array(
                        @errors::resource_not_registered_details(@namespace, @contract_name)
                    )
                },
                _ => panic_with_byte_array(
                    @errors::resource_conflict(
                        @format!("{}-{}", @namespace, @contract_name), @"contract"
                    )
                )
            }
        }

        fn init_contract(ref self: ContractState, selector: felt252, init_calldata: Span<felt252>) {
            if let Resource::Contract((contract_address, _)) = self.resources.read(selector) {
                if self.initialized_contracts.read(selector) {
                    let dispatcher = IContractDispatcher { contract_address };
                    panic_with_byte_array(
                        @errors::contract_already_initialized(@dispatcher.dojo_name())
                    );
                } else {
                    self.assert_caller_permissions(selector, Permission::Owner);

                    // For the init, to ensure only the world can call the init function,
                    // the verification is done in the init function of the contract that is
                    // injected by the plugin.
                    // <crates/compiler/src/plugin/attribute_macros/contract.rs#L275>

                    starknet::syscalls::call_contract_syscall(
                        contract_address, DOJO_INIT_SELECTOR, init_calldata
                    )
                        .unwrap_syscall();

                    self.initialized_contracts.write(selector, true);

                    self.emit(ContractInitialized { selector, init_calldata });
                }
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{selector}"), @"contract")
                );
            }
        }

        fn uuid(ref self: ContractState) -> usize {
            let current = self.nonce.read();
            self.nonce.write(current + 1);
            current
        }

        fn emit_event(
            ref self: ContractState,
            event_selector: felt252,
            keys: Span<felt252>,
            values: Span<felt252>,
            historical: bool
        ) {
            if let Resource::Event((_, _)) = self.resources.read(event_selector) {
                self.assert_caller_permissions(event_selector, Permission::Writer);

                self
                    .emit(
                        EventEmitted {
                            event_selector,
                            system_address: get_caller_address(),
                            historical,
                            keys,
                            values,
                        }
                    );
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{event_selector}"), @"event")
                );
            }
        }

        fn entity(
            self: @ContractState, model_selector: felt252, index: ModelIndex, layout: Layout
        ) -> Span<felt252> {
            match index {
                ModelIndex::Keys(keys) => {
                    let entity_id = entity_id_from_keys(keys);
                    storage::entity_model::read_model_entity(model_selector, entity_id, layout)
                },
                ModelIndex::Id(entity_id) => {
                    storage::entity_model::read_model_entity(model_selector, entity_id, layout)
                },
                ModelIndex::MemberId((
                    entity_id, member_id
                )) => {
                    storage::entity_model::read_model_member(
                        model_selector, entity_id, member_id, layout
                    )
                }
            }
        }

        fn set_entity(
            ref self: ContractState,
            model_selector: felt252,
            index: ModelIndex,
            values: Span<felt252>,
            layout: Layout
        ) {
            if let Resource::Model((_, _)) = self.resources.read(model_selector) {
                self.assert_caller_permissions(model_selector, Permission::Writer);
                self.set_entity_internal(model_selector, index, values, layout);
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{model_selector}"), @"model")
                );
            }
        }

        fn delete_entity(
            ref self: ContractState, model_selector: felt252, index: ModelIndex, layout: Layout
        ) {
            if let Resource::Model((_, _)) = self.resources.read(model_selector) {
                self.assert_caller_permissions(model_selector, Permission::Writer);
                self.delete_entity_internal(model_selector, index, layout);
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{model_selector}"), @"model")
                );
            }
        }

        fn resource(self: @ContractState, selector: felt252) -> Resource {
            self.resources.read(selector)
        }
    }

    #[abi(embed_v0)]
    impl UpgradeableWorld of IUpgradeableWorld<ContractState> {
        fn upgrade(ref self: ContractState, new_class_hash: ClassHash) {
            assert(new_class_hash.is_non_zero(), 'invalid class_hash');

            if !self.is_caller_world_owner() {
                panic_with_byte_array(@errors::not_owner_upgrade(get_caller_address(), WORLD));
            }

            replace_class_syscall(new_class_hash).unwrap();

            self.emit(WorldUpgraded { class_hash: new_class_hash });
        }
    }

    #[generate_trait]
    impl SelfImpl of SelfTrait {
        #[inline(always)]
        /// Indicates if the caller is the owner of the world.
        fn is_caller_world_owner(self: @ContractState) -> bool {
            self.is_owner(WORLD, get_caller_address())
        }

        /// Asserts the caller has the required permissions for a resource, following the
        /// permissions hierarchy:
        /// 1. World Owner
        /// 2. Namespace Owner
        /// 3. Resource Owner
        /// [if writer]
        /// 4. Namespace Writer
        /// 5. Resource Writer
        ///
        /// This function is expected to be called very often as it's used to check permissions
        /// for all the resource access in the system.
        /// For this reason, here are the following optimizations:
        ///     * Use several single `if` because it seems more efficient than a big one with
        ///       several conditions based on how cairo is lowered to sierra.
        ///     * Sort conditions by order of probability so once a condition is met, the function
        ///       returns.
        ///
        /// # Arguments
        ///   * `resource_selector` - the selector of the resource.
        ///   * `permission` - the required permission.
        fn assert_caller_permissions(
            self: @ContractState, resource_selector: felt252, permission: Permission
        ) {
            let caller = get_caller_address();

            if permission == Permission::Writer {
                if self.is_writer(resource_selector, caller) {
                    return;
                }
            }

            if self.is_owner(resource_selector, caller) {
                return;
            }

            if self.is_caller_world_owner() {
                return;
            }

            // At this point, [`Resource::Contract`] and [`Resource::Model`] requires extra checks
            // by switching to the namespace hash being the resource selector.
            let namespace_hash = match self.resources.read(resource_selector) {
                Resource::Contract((_, namespace_hash)) => { namespace_hash },
                Resource::Model((_, namespace_hash)) => { namespace_hash },
                Resource::Unregistered => {
                    panic_with_byte_array(@errors::resource_not_registered(resource_selector))
                },
                _ => self.panic_with_details(caller, resource_selector, permission)
            };

            if permission == Permission::Writer {
                if self.is_writer(namespace_hash, caller) {
                    return;
                }
            }

            if self.is_owner(namespace_hash, caller) {
                return;
            }

            self.panic_with_details(caller, resource_selector, permission)
        }

        /// Asserts the name is valid according to the naming convention.
        fn assert_name(self: @ContractState, name: @ByteArray) {
            if !dojo::utils::is_name_valid(name) {
                panic_with_byte_array(@errors::invalid_naming("Name", name))
            }
        }

        /// Asserts the namespace is valid according to the naming convention.
        fn assert_namespace(self: @ContractState, namespace: @ByteArray) {
            if !dojo::utils::is_name_valid(namespace) {
                panic_with_byte_array(@errors::invalid_naming("Namespace", namespace))
            }
        }

        /// Panics with the caller details.
        ///
        /// # Arguments
        ///   * `caller` - the address of the caller.
        ///   * `resource_selector` - the selector of the resource.
        ///   * `permission` - the required permission.
        fn panic_with_details(
            self: @ContractState,
            caller: ContractAddress,
            resource_selector: felt252,
            permission: Permission
        ) -> core::never {
            let resource_name = match self.resources.read(resource_selector) {
                Resource::Contract((
                    contract_address, _
                )) => {
                    let d = IContractDispatcher { contract_address };
                    format!("contract (or its namespace) `{}`", d.dojo_name())
                },
                Resource::Event((
                    contract_address, _
                )) => {
                    let d = IEventDispatcher { contract_address };
                    format!("event (or its namespace) `{}`", d.dojo_name())
                },
                Resource::Model((
                    contract_address, _
                )) => {
                    let d = IModelDispatcher { contract_address };
                    format!("model (or its namespace) `{}`", d.dojo_name())
                },
                Resource::Namespace(ns) => { format!("namespace `{}`", ns) },
                Resource::World => { format!("world") },
                Resource::Unregistered => { panic!("Unreachable") }
            };

            let caller_name = if caller == get_tx_info().account_contract_address {
                format!("Account `{:?}`", caller)
            } else {
                // If the caller is not a dojo contract, the `d.selector()` will fail. In the
                // future we should use the SRC5 to first query the contract to see if
                // it implements the `IDescriptor` interface.
                // For now, we just assume that the caller is a dojo contract as it's 100% of
                // the dojo use cases at the moment.
                // If the contract is not an account or a dojo contract, tests will display
                // "CONTRACT_NOT_DEPLOYED" as the error message. In production, the error message
                // will display "ENTRYPOINT_NOT_FOUND".
                let d = IContractDispatcher { contract_address: caller };
                format!("Contract `{}`", d.dojo_name())
            };

            panic_with_byte_array(
                @format!("{} does NOT have {} role on {}", caller_name, permission, resource_name)
            )
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
                    storage::entity_model::write_model_entity(
                        model_selector, entity_id, values, layout
                    );
                    self.emit(StoreSetRecord { table: model_selector, keys, values, entity_id });
                },
                ModelIndex::Id(entity_id) => {
                    storage::entity_model::write_model_entity(
                        model_selector, entity_id, values, layout
                    );
                    self.emit(StoreUpdateRecord { table: model_selector, entity_id, values });
                },
                ModelIndex::MemberId((
                    entity_id, member_selector
                )) => {
                    storage::entity_model::write_model_member(
                        model_selector, entity_id, member_selector, values, layout
                    );
                    self
                        .emit(
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
                    storage::entity_model::delete_model_entity(model_selector, entity_id, layout);
                    self.emit(StoreDelRecord { table: model_selector, entity_id });
                },
                ModelIndex::Id(entity_id) => {
                    storage::entity_model::delete_model_entity(model_selector, entity_id, layout);
                    self.emit(StoreDelRecord { table: model_selector, entity_id });
                },
                ModelIndex::MemberId(_) => { panic_with_felt252(errors::DELETE_ENTITY_MEMBER); }
            }
        }

        /// Returns the hash of the internal namespace for a dojo world.
        fn world_internal_namespace(self: @ContractState) -> (ByteArray, felt252) {
            let name = "__DOJO__";
            let hash = bytearray_hash(@name);

            (name, hash)
        }
    }
}
