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
        syscalls::{deploy_syscall, replace_class_syscall, get_class_hash_at_syscall},
        SyscallResultTrait, storage::Map,
    };
    pub use starknet::storage::{
        StorageMapReadAccess, StorageMapWriteAccess, StoragePointerReadAccess,
        StoragePointerWriteAccess,
    };

    use dojo::world::errors;
    use dojo::contract::components::upgradeable::{
        IUpgradeableDispatcher, IUpgradeableDispatcherTrait,
    };
    use dojo::meta::{
        Layout, IStoredResourceDispatcher, IStoredResourceDispatcherTrait,
        IDeployedResourceDispatcher, IDeployedResourceDispatcherTrait, LayoutCompareTrait,
        IDeployedResourceLibraryDispatcher, TyCompareTrait,
    };
    use dojo::model::{Model, ResourceMetadata, metadata, ModelIndex};
    use dojo::storage;
    use dojo::utils::{
        entity_id_from_serialized_keys, bytearray_hash, selector_from_namespace_and_name,
    };
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
        ExternalContractRegistered: ExternalContractRegistered,
        ExternalContractUpgraded: ExternalContractUpgraded,
        ModelUpgraded: ModelUpgraded,
        EventUpgraded: EventUpgraded,
        ContractUpgraded: ContractUpgraded,
        ContractInitialized: ContractInitialized,
        LibraryRegistered: LibraryRegistered,
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
        pub name: ByteArray,
        #[key]
        pub namespace: ByteArray,
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
    pub struct ExternalContractRegistered {
        #[key]
        pub namespace: ByteArray,
        #[key]
        pub contract_name: ByteArray,
        #[key]
        pub instance_name: ByteArray,
        #[key]
        pub contract_selector: felt252,
        pub class_hash: ClassHash,
        pub contract_address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    pub struct ExternalContractUpgraded {
        #[key]
        pub namespace: ByteArray,
        #[key]
        pub instance_name: ByteArray,
        #[key]
        pub contract_selector: felt252,
        pub class_hash: ClassHash,
        pub contract_address: ContractAddress,
    }

    #[derive(Drop, starknet::Event)]
    pub struct LibraryRegistered {
        #[key]
        pub name: ByteArray,
        #[key]
        pub namespace: ByteArray,
        pub class_hash: ClassHash,
    }

    #[derive(Drop, starknet::Event)]
    pub struct MetadataUpdate {
        #[key]
        pub resource: felt252,
        pub uri: ByteArray,
        pub hash: felt252,
    }

    #[derive(Drop, starknet::Event)]
    pub struct NamespaceRegistered {
        #[key]
        pub namespace: ByteArray,
        pub hash: felt252,
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
        pub selector: felt252,
        #[key]
        pub entity_id: felt252,
        pub keys: Span<felt252>,
        pub values: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreUpdateRecord {
        #[key]
        pub selector: felt252,
        #[key]
        pub entity_id: felt252,
        pub values: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreUpdateMember {
        #[key]
        pub selector: felt252,
        #[key]
        pub entity_id: felt252,
        #[key]
        pub member_selector: felt252,
        pub values: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    pub struct StoreDelRecord {
        #[key]
        pub selector: felt252,
        #[key]
        pub entity_id: felt252,
    }

    #[derive(Drop, starknet::Event)]
    pub struct WriterUpdated {
        #[key]
        pub resource: felt252,
        #[key]
        pub contract: ContractAddress,
        pub value: bool,
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
        pub selector: felt252,
        #[key]
        pub system_address: ContractAddress,
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
        owner_count: Map::<felt252, u64>,
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
        self.write_ownership(internal_ns_hash, creator, true);

        self.resources.write(WORLD, Resource::World);
        self.write_ownership(WORLD, creator, true);

        // This model doesn't need to have the class hash or the contract address
        // set since they are manually controlled by the world contract.
        self
            .resources
            .write(
                metadata::resource_metadata_selector(internal_ns_hash),
                Resource::Model(
                    (metadata::default_address(), metadata::default_class_hash().into()),
                ),
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
            layout: Layout,
        ) {
            self.set_entity_internal(model_selector, index, values, layout);
        }

        fn delete_entity_test(
            ref self: ContractState, model_selector: felt252, index: ModelIndex, layout: Layout,
        ) {
            self.delete_entity_internal(model_selector, index, layout);
        }

        fn emit_event_test(
            ref self: ContractState,
            event_selector: felt252,
            keys: Span<felt252>,
            values: Span<felt252>,
        ) {
            self
                .emit(
                    EventEmitted {
                        selector: event_selector,
                        system_address: get_caller_address(),
                        keys,
                        values,
                    },
                );
        }

        fn dojo_contract_address(
            self: @ContractState, contract_selector: felt252,
        ) -> ContractAddress {
            match self.resources.read(contract_selector) {
                Resource::Contract((a, _)) => a,
                Resource::ExternalContract((a, _)) => a,
                _ => core::panics::panic_with_byte_array(
                    @format!("Contract/ExternalContract not registered: {}", contract_selector),
                ),
            }
        }
    }

    #[abi(embed_v0)]
    impl World of IWorld<ContractState> {
        fn metadata(self: @ContractState, resource_selector: felt252) -> ResourceMetadata {
            let (_, internal_ns_hash) = self.world_internal_namespace();

            let mut values = storage::entity_model::read_model_entity(
                metadata::resource_metadata_selector(internal_ns_hash),
                entity_id_from_serialized_keys([resource_selector].span()),
                Model::<ResourceMetadata>::layout(),
            );

            let mut keys = [resource_selector].span();

            match Model::<ResourceMetadata>::from_serialized(keys, values) {
                Option::Some(x) => x,
                Option::None => panic!("Model `ResourceMetadata`: deserialization failed."),
            }
        }

        fn set_metadata(ref self: ContractState, metadata: ResourceMetadata) {
            self.assert_caller_permissions(metadata.resource_id, Permission::Owner);

            let (_, internal_ns_hash) = self.world_internal_namespace();

            storage::entity_model::write_model_entity(
                metadata::resource_metadata_selector(internal_ns_hash),
                entity_id_from_serialized_keys([metadata.resource_id].span()),
                metadata.serialized_values(),
                Model::<ResourceMetadata>::layout(),
            );

            self
                .emit(
                    MetadataUpdate {
                        resource: metadata.resource_id,
                        uri: metadata.metadata_uri,
                        hash: metadata.metadata_hash,
                    },
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

            self.write_ownership(resource, address, true);

            self.emit(OwnerUpdated { contract: address, resource, value: true });
        }

        fn revoke_owner(ref self: ContractState, resource: felt252, address: ContractAddress) {
            if self.resources.read(resource).is_unregistered() {
                panic_with_byte_array(@errors::resource_not_registered(resource));
            }

            self.assert_caller_permissions(resource, Permission::Owner);

            self.write_ownership(resource, address, false);

            self.emit(OwnerUpdated { contract: address, resource, value: false });
        }

        fn owners_count(self: @ContractState, resource: felt252) -> u64 {
            self.owner_count.read(resource)
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

            let event = IDeployedResourceDispatcher { contract_address };
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
            self.write_ownership(event_selector, caller, true);

            self
                .emit(
                    EventRegistered {
                        name: event_name.clone(),
                        namespace: namespace.clone(),
                        address: contract_address,
                        class_hash,
                    },
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

            let event = IDeployedResourceDispatcher { contract_address: new_contract_address };
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
                        @errors::resource_not_registered_details(@namespace, @event_name),
                    )
                },
                _ => panic_with_byte_array(
                    @errors::resource_conflict(
                        @format!("{}-{}", @namespace, @event_name), @"event",
                    ),
                ),
            };

            self
                .assert_resource_upgradability(
                    @namespace, @event_name, prev_address, new_contract_address,
                );

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
                    },
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

            let model = IDeployedResourceDispatcher { contract_address };
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
            self.write_ownership(model_selector, caller, true);

            self
                .emit(
                    ModelRegistered {
                        name: model_name.clone(),
                        namespace: namespace.clone(),
                        address: contract_address,
                        class_hash,
                    },
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

            let model = IDeployedResourceDispatcher { contract_address: new_contract_address };
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
                        @errors::resource_not_registered_details(@namespace, @model_name),
                    )
                },
                _ => panic_with_byte_array(
                    @errors::resource_conflict(
                        @format!("{}-{}", @namespace, @model_name), @"model",
                    ),
                ),
            };

            self
                .assert_resource_upgradability(
                    @namespace, @model_name, prev_address, new_contract_address,
                );

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
                    },
                );
        }

        fn register_namespace(ref self: ContractState, namespace: ByteArray) {
            self.assert_namespace(@namespace);

            let caller = get_caller_address();

            let hash = bytearray_hash(@namespace);

            match self.resources.read(hash) {
                Resource::Namespace => panic_with_byte_array(
                    @errors::namespace_already_registered(@namespace),
                ),
                Resource::Unregistered => {
                    self.resources.write(hash, Resource::Namespace(namespace.clone()));
                    self.write_ownership(hash, caller, true);

                    self.emit(NamespaceRegistered { namespace, hash });
                },
                _ => {
                    panic_with_byte_array(@errors::resource_conflict(@namespace, @"namespace"));
                },
            };
        }

        fn register_contract(
            ref self: ContractState, salt: felt252, namespace: ByteArray, class_hash: ClassHash,
        ) -> ContractAddress {
            let caller = get_caller_address();

            let (contract_address, _) = deploy_syscall(class_hash, salt, [].span(), false)
                .unwrap_syscall();

            let namespace_hash = bytearray_hash(@namespace);

            let contract = IDeployedResourceDispatcher { contract_address };
            let contract_name = contract.dojo_name();
            let contract_selector = selector_from_namespace_and_name(
                namespace_hash, @contract_name,
            );

            self.assert_name(@contract_name);

            let maybe_existing_contract = self.resources.read(contract_selector);
            if !maybe_existing_contract.is_unregistered() {
                panic_with_byte_array(
                    @errors::contract_already_registered(@namespace, @contract_name),
                );
            }

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_permissions(namespace_hash, Permission::Owner);

            self.write_ownership(contract_selector, caller, true);
            self
                .resources
                .write(contract_selector, Resource::Contract((contract_address, namespace_hash)));

            self
                .emit(
                    ContractRegistered {
                        salt, class_hash, address: contract_address, namespace, name: contract_name,
                    },
                );

            contract_address
        }

        fn upgrade_contract(
            ref self: ContractState, namespace: ByteArray, class_hash: ClassHash,
        ) -> ClassHash {
            // Only contracts use an external salt during registration. To ensure the
            // upgrade can also be done into a multicall, we combine the transaction hash
            // and the namespace hash since we can't have the same class hash registered more than
            // once in the same namespace.
            let salt = core::poseidon::poseidon_hash_span(
                [
                    starknet::get_tx_info().unbox().transaction_hash,
                    dojo::utils::bytearray_hash(@namespace),
                ]
                    .span(),
            );

            let (new_contract_address, _) = deploy_syscall(class_hash, salt, [].span(), false)
                .unwrap_syscall();

            let namespace_hash = bytearray_hash(@namespace);

            let contract = IDeployedResourceDispatcher { contract_address: new_contract_address };
            let contract_name = contract.dojo_name();
            let contract_selector = selector_from_namespace_and_name(
                namespace_hash, @contract_name,
            );

            // If namespace and name are the same, the contract is already registered and we
            // can upgrade it.
            match self.resources.read(contract_selector) {
                Resource::Contract((
                    contract_address, _,
                )) => {
                    self.assert_caller_permissions(contract_selector, Permission::Owner);

                    IUpgradeableDispatcher { contract_address }.upgrade(class_hash);
                    self.emit(ContractUpgraded { class_hash, selector: contract_selector });

                    class_hash
                },
                Resource::Unregistered => {
                    panic_with_byte_array(
                        @errors::resource_not_registered_details(@namespace, @contract_name),
                    )
                },
                _ => panic_with_byte_array(
                    @errors::resource_conflict(
                        @format!("{}-{}", @namespace, @contract_name), @"contract",
                    ),
                ),
            }
            // class_hash will be retrieved with get_class_hash_at_syscall, so no need to update
        // resource.
        }

        fn init_contract(ref self: ContractState, selector: felt252, init_calldata: Span<felt252>) {
            if let Resource::Contract((contract_address, _)) = self.resources.read(selector) {
                if self.initialized_contracts.read(selector) {
                    let dispatcher = IDeployedResourceDispatcher { contract_address };
                    panic_with_byte_array(
                        @errors::contract_already_initialized(@dispatcher.dojo_name()),
                    );
                } else {
                    self.assert_caller_permissions(selector, Permission::Owner);

                    // For the init, to ensure only the world can call the init function,
                    // the verification is done in the init function of the contract that is
                    // injected by the plugin.
                    // <crates/compiler/src/plugin/attribute_macros/contract.rs#L275>

                    starknet::syscalls::call_contract_syscall(
                        contract_address, DOJO_INIT_SELECTOR, init_calldata,
                    )
                        .unwrap_syscall();

                    self.initialized_contracts.write(selector, true);

                    self.emit(ContractInitialized { selector, init_calldata });
                }
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{selector}"), @"contract"),
                );
            }
        }

        fn register_external_contract(
            ref self: ContractState,
            namespace: ByteArray,
            contract_name: ByteArray,
            instance_name: ByteArray,
            contract_address: ContractAddress,
        ) {
            let caller = get_caller_address();
            let class_hash = get_class_hash_at_syscall(contract_address).unwrap_syscall();

            self.assert_name(@instance_name);

            let namespace_hash = bytearray_hash(@namespace);
            let contract_selector = selector_from_namespace_and_name(
                namespace_hash, @instance_name,
            );

            let maybe_existing_contract = self.resources.read(contract_selector);
            if !maybe_existing_contract.is_unregistered() {
                panic_with_byte_array(
                    @errors::external_contract_already_registered(
                        @namespace, @contract_name, @instance_name,
                    ),
                );
            }

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_permissions(namespace_hash, Permission::Owner);

            self.write_ownership(contract_selector, caller, true);
            self
                .resources
                .write(
                    contract_selector,
                    Resource::ExternalContract((contract_address, namespace_hash)),
                );

            self
                .emit(
                    ExternalContractRegistered {
                        namespace,
                        contract_name,
                        instance_name,
                        contract_selector,
                        class_hash,
                        contract_address,
                    },
                );
        }

        fn upgrade_external_contract(
            ref self: ContractState,
            namespace: ByteArray,
            instance_name: ByteArray,
            contract_address: ContractAddress,
        ) {
            let class_hash = get_class_hash_at_syscall(contract_address).unwrap_syscall();

            let namespace_hash = bytearray_hash(@namespace);
            let contract_selector = selector_from_namespace_and_name(
                namespace_hash, @instance_name,
            );

            match self.resources.read(contract_selector) {
                Resource::ExternalContract(_) => {
                    self.assert_caller_permissions(contract_selector, Permission::Owner);

                    self
                        .resources
                        .write(
                            contract_selector,
                            Resource::ExternalContract((contract_address, namespace_hash)),
                        );

                    self
                        .emit(
                            ExternalContractUpgraded {
                                namespace,
                                instance_name,
                                contract_selector,
                                class_hash,
                                contract_address,
                            },
                        );
                },
                Resource::Unregistered => panic_with_byte_array(
                    @errors::resource_not_registered_details(@namespace, @instance_name),
                ),
                _ => panic_with_byte_array(
                    @errors::resource_conflict(
                        @format!("{}-{}", @namespace, @instance_name), @"external contract",
                    ),
                ),
            }
        }

        fn register_library(
            ref self: ContractState,
            namespace: ByteArray,
            class_hash: ClassHash,
            name: ByteArray,
            version: ByteArray,
        ) -> ClassHash {
            let caller = get_caller_address();

            let namespace_hash = bytearray_hash(@namespace);

            let contract_name = format!("{}_v{}", name, version);
            self.assert_name(@contract_name);

            let contract_selector = selector_from_namespace_and_name(
                namespace_hash, @contract_name,
            );

            let maybe_existing_library = self.resources.read(contract_selector);
            if !maybe_existing_library.is_unregistered() {
                panic_with_byte_array(
                    @errors::library_already_registered(@namespace, @contract_name),
                );
            }

            if !self.is_namespace_registered(namespace_hash) {
                panic_with_byte_array(@errors::namespace_not_registered(@namespace));
            }

            self.assert_caller_permissions(namespace_hash, Permission::Owner);

            self.write_ownership(contract_selector, caller, true);
            self
                .resources
                .write(contract_selector, Resource::Library((class_hash, namespace_hash)));

            self.emit(LibraryRegistered { class_hash, namespace, name: contract_name });

            class_hash
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
        ) {
            if let Resource::Event((_, _)) = self.resources.read(event_selector) {
                self.assert_caller_permissions(event_selector, Permission::Writer);

                self
                    .emit(
                        EventEmitted {
                            selector: event_selector,
                            system_address: get_caller_address(),
                            keys,
                            values,
                        },
                    );
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{event_selector}"), @"event"),
                );
            }
        }

        fn emit_events(
            ref self: ContractState,
            event_selector: felt252,
            keys: Span<Span<felt252>>,
            values: Span<Span<felt252>>,
        ) {
            if let Resource::Event((_, _)) = self.resources.read(event_selector) {
                self.assert_caller_permissions(event_selector, Permission::Writer);

                if keys.len() != values.len() {
                    panic_with_byte_array(
                        @errors::lengths_mismatch(@"keys", @"values", @"emit_events"),
                    );
                }

                let mut i = 0;
                loop {
                    if i >= keys.len() {
                        break;
                    }

                    self
                        .emit(
                            EventEmitted {
                                selector: event_selector,
                                system_address: get_caller_address(),
                                keys: *keys[i],
                                values: *values[i],
                            },
                        );

                    i += 1;
                }
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{event_selector}"), @"event"),
                );
            }
        }

        fn entity(
            self: @ContractState, model_selector: felt252, index: ModelIndex, layout: Layout,
        ) -> Span<felt252> {
            self.get_entity_internal(model_selector, index, layout)
        }

        fn entities(
            self: @ContractState,
            model_selector: felt252,
            indexes: Span<ModelIndex>,
            layout: Layout,
        ) -> Span<Span<felt252>> {
            let mut models: Array<Span<felt252>> = array![];

            for i in indexes {
                models.append(self.get_entity_internal(model_selector, *i, layout));
            };

            models.span()
        }

        fn set_entity(
            ref self: ContractState,
            model_selector: felt252,
            index: ModelIndex,
            values: Span<felt252>,
            layout: Layout,
        ) {
            if let Resource::Model((_, _)) = self.resources.read(model_selector) {
                self.assert_caller_permissions(model_selector, Permission::Writer);
                self.set_entity_internal(model_selector, index, values, layout);
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{model_selector}"), @"model"),
                );
            }
        }

        fn set_entities(
            ref self: ContractState,
            model_selector: felt252,
            indexes: Span<ModelIndex>,
            values: Span<Span<felt252>>,
            layout: Layout,
        ) {
            if indexes.len() != values.len() {
                panic_with_byte_array(
                    @errors::lengths_mismatch(@"indexes", @"values", @"set_entities"),
                );
            }

            if let Resource::Model((_, _)) = self.resources.read(model_selector) {
                self.assert_caller_permissions(model_selector, Permission::Writer);

                let mut i = 0;
                loop {
                    if i >= indexes.len() {
                        break;
                    }

                    self.set_entity_internal(model_selector, *indexes[i], *values[i], layout);

                    i += 1;
                };
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{model_selector}"), @"model"),
                );
            }
        }

        fn delete_entity(
            ref self: ContractState, model_selector: felt252, index: ModelIndex, layout: Layout,
        ) {
            if let Resource::Model((_, _)) = self.resources.read(model_selector) {
                self.assert_caller_permissions(model_selector, Permission::Writer);
                self.delete_entity_internal(model_selector, index, layout);
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{model_selector}"), @"model"),
                );
            }
        }

        fn delete_entities(
            ref self: ContractState,
            model_selector: felt252,
            indexes: Span<ModelIndex>,
            layout: Layout,
        ) {
            if let Resource::Model((_, _)) = self.resources.read(model_selector) {
                self.assert_caller_permissions(model_selector, Permission::Writer);

                for i in indexes {
                    self.delete_entity_internal(model_selector, *i, layout);
                }
            } else {
                panic_with_byte_array(
                    @errors::resource_conflict(@format!("{model_selector}"), @"model"),
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
        /// Update the ownership status of a resource owner.
        ///
        /// # Arguments
        ///   * `resource` - The selector of the resource.
        ///   * `owner` - The owner address.
        ///   $ `is_owner` - true to set `owner` as a new resource owner,
        ///                  false to remove the `owner` from the resource owners.
        fn write_ownership(
            ref self: ContractState, resource: felt252, owner: ContractAddress, is_owner: bool,
        ) {
            let was_owner = self.owners.read((resource, owner));

            if was_owner != is_owner {
                let new_count = if is_owner {
                    self.owner_count.read(resource) + 1
                } else {
                    self.owner_count.read(resource) - 1
                };

                self.owner_count.write(resource, new_count);
                self.owners.write((resource, owner), is_owner);
            }
        }

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
            self: @ContractState, resource_selector: felt252, permission: Permission,
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

            // At this point, [`Resource::Contract`], [`Resource::ExternalContract`],
            // [`Resource::Model`] and [`Resource::Event`] require extra checks
            // by switching to the namespace hash being the resource selector.
            let namespace_hash = match self.resources.read(resource_selector) {
                Resource::Contract((_, namespace_hash)) => { namespace_hash },
                Resource::ExternalContract((_, namespace_hash)) => { namespace_hash },
                Resource::Model((_, namespace_hash)) => { namespace_hash },
                Resource::Event((_, namespace_hash)) => { namespace_hash },
                Resource::Unregistered => {
                    panic_with_byte_array(@errors::resource_not_registered(resource_selector))
                },
                _ => self.panic_with_details(caller, resource_selector, permission),
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

        /// Panics if a resource is not upgradable.
        ///
        /// Upgradable means:
        /// - the layout type must remain the same (Struct or Fixed),
        /// - existing fields cannot be changed or moved inside the resource,
        /// - new fields can only be appended at the end of the resource.
        ///
        /// # Arguments
        ///   * `namespace` - the namespace of the resource.
        ///   * `name` - the name of the resource.
        ///   * `prev_address` - the address of the current resource.
        ///   * `new_address` - the address of the newly deployed resource.
        ///
        fn assert_resource_upgradability(
            self: @ContractState,
            namespace: @ByteArray,
            name: @ByteArray,
            prev_address: ContractAddress,
            new_address: ContractAddress,
        ) {
            let resource = IStoredResourceDispatcher { contract_address: prev_address };
            let old_layout = resource.layout();
            let old_schema = resource.schema();

            let new_resource = IStoredResourceDispatcher { contract_address: new_address };
            let new_layout = new_resource.layout();
            let new_schema = new_resource.schema();

            if !new_layout.is_same_type_of(@old_layout) {
                panic_with_byte_array(@errors::invalid_resource_layout_upgrade(namespace, name));
            }

            if let Layout::Fixed(_) = new_layout {
                panic_with_byte_array(@errors::packed_layout_cannot_be_upgraded(namespace, name));
            }

            if !new_schema.is_an_upgrade_of(@old_schema) {
                panic_with_byte_array(@errors::invalid_resource_schema_upgrade(namespace, name));
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
            permission: Permission,
        ) -> core::never {
            let resource_name = match self.resources.read(resource_selector) {
                Resource::Contract((
                    contract_address, _,
                )) => {
                    let d = IDeployedResourceDispatcher { contract_address };
                    format!("contract (or its namespace) `{}`", d.dojo_name())
                },
                Resource::ExternalContract((
                    contract_address, _,
                )) => { format!("external contract (at 0x{:x})", contract_address) },
                Resource::Event((
                    contract_address, _,
                )) => {
                    let d = IDeployedResourceDispatcher { contract_address };
                    format!("event (or its namespace) `{}`", d.dojo_name())
                },
                Resource::Model((
                    contract_address, _,
                )) => {
                    let d = IDeployedResourceDispatcher { contract_address };
                    format!("model (or its namespace) `{}`", d.dojo_name())
                },
                Resource::Namespace(ns) => { format!("namespace `{}`", ns) },
                Resource::World => { format!("world") },
                Resource::Unregistered => { panic!("Unreachable") },
                Resource::Library((
                    class_hash, _,
                )) => {
                    let d = IDeployedResourceLibraryDispatcher { class_hash };
                    format!("library (or its namespace) `{}`", d.dojo_name())
                },
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
                let d = IDeployedResourceDispatcher { contract_address: caller };
                format!("Contract `{}`", d.dojo_name())
            };

            panic_with_byte_array(
                @format!("{} does NOT have {} role on {}", caller_name, permission, resource_name),
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
                _ => false,
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
            layout: Layout,
        ) {
            match index {
                ModelIndex::Keys(keys) => {
                    let entity_id = entity_id_from_serialized_keys(keys);
                    storage::entity_model::write_model_entity(
                        model_selector, entity_id, values, layout,
                    );
                    self.emit(StoreSetRecord { selector: model_selector, keys, values, entity_id });
                },
                ModelIndex::Id(entity_id) => {
                    storage::entity_model::write_model_entity(
                        model_selector, entity_id, values, layout,
                    );
                    self.emit(StoreUpdateRecord { selector: model_selector, entity_id, values });
                },
                ModelIndex::MemberId((
                    entity_id, member_selector,
                )) => {
                    storage::entity_model::write_model_member(
                        model_selector, entity_id, member_selector, values, layout,
                    );
                    self
                        .emit(
                            StoreUpdateMember {
                                selector: model_selector, entity_id, member_selector, values,
                            },
                        );
                },
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
            ref self: ContractState, model_selector: felt252, index: ModelIndex, layout: Layout,
        ) {
            match index {
                ModelIndex::Keys(keys) => {
                    let entity_id = entity_id_from_serialized_keys(keys);
                    storage::entity_model::delete_model_entity(model_selector, entity_id, layout);
                    self.emit(StoreDelRecord { selector: model_selector, entity_id });
                },
                ModelIndex::Id(entity_id) => {
                    storage::entity_model::delete_model_entity(model_selector, entity_id, layout);
                    self.emit(StoreDelRecord { selector: model_selector, entity_id });
                },
                ModelIndex::MemberId(_) => { panic_with_felt252(errors::DELETE_ENTITY_MEMBER); },
            }
        }

        /// Gets the model values for the given entity.
        ///
        /// # Arguments
        ///
        /// * `model_selector` - The selector of the model to be retrieved.
        /// * `index` - The entity/member to read for the given model.
        /// * `layout` - The memory layout of the model.
        fn get_entity_internal(
            self: @ContractState, model_selector: felt252, index: ModelIndex, layout: Layout,
        ) -> Span<felt252> {
            match index {
                ModelIndex::Keys(keys) => {
                    let entity_id = entity_id_from_serialized_keys(keys);
                    storage::entity_model::read_model_entity(model_selector, entity_id, layout)
                },
                ModelIndex::Id(entity_id) => {
                    storage::entity_model::read_model_entity(model_selector, entity_id, layout)
                },
                ModelIndex::MemberId((
                    entity_id, member_id,
                )) => {
                    storage::entity_model::read_model_member(
                        model_selector, entity_id, member_id, layout,
                    )
                },
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
