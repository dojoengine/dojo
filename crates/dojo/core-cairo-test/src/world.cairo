use core::option::OptionTrait;
use core::result::ResultTrait;
use core::traits::{Into, TryInto};

use starknet::{ContractAddress, ClassHash, syscalls::deploy_syscall};

use dojo::world::{world, IWorldDispatcher, IWorldDispatcherTrait, WorldStorageTrait, WorldStorage};

/// In Cairo test runner, all the classes are expected to be declared already.
/// If a contract belong to an other crate, it must be added to the `build-external-contract`,
/// event for testing, since Scarb does not do that automatically anymore.
#[derive(Drop)]
pub enum TestResource {
    Event: ClassHash,
    Model: ClassHash,
    Contract: ContractDef,
}

#[derive(Drop)]
pub struct NamespaceDef {
    pub namespace: ByteArray,
    pub resources: Span<TestResource>,
}

#[derive(Drop)]
pub struct ContractDef {
    /// Class hash, use `felt252` instead of `ClassHash` as TEST_CLASS_HASH is a `felt252`.
    pub class_hash: felt252,
    /// Name of the contract.
    pub name: ByteArray,
    /// Calldata for dojo_init.
    pub init_calldata: Span<felt252>,
    /// Selectors of the resources that the contract is granted writer access to.
    pub writer_of: Span<felt252>,
    /// Selector of the resource that the contract is the owner of.
    pub owner_of: Span<felt252>,
}

#[generate_trait]
pub impl ContractDefImpl of ContractDefTrait {
    fn new(class_hash: felt252, name: ByteArray) -> ContractDef {
        ContractDef {
            class_hash, name, init_calldata: [].span(), writer_of: [].span(), owner_of: [].span()
        }
    }

    fn with_init_calldata(mut self: ContractDef, init_calldata: Span<felt252>) -> ContractDef {
        self.init_calldata = init_calldata;
        self
    }

    fn with_writer_of(mut self: ContractDef, writer_of: Span<felt252>) -> ContractDef {
        self.writer_of = writer_of;
        self
    }

    fn with_owner_of(mut self: ContractDef, owner_of: Span<felt252>) -> ContractDef {
        self.owner_of = owner_of;
        self
    }
}

/// Deploy classhash with calldata for constructor
///
/// # Arguments
///
/// * `class_hash` - Class to deploy
/// * `calldata` - calldata for constructor
///
/// # Returns
/// * address of contract deployed
pub fn deploy_contract(class_hash: felt252, calldata: Span<felt252>) -> ContractAddress {
    let (contract, _) = starknet::syscalls::deploy_syscall(
        class_hash.try_into().unwrap(), 0, calldata, false
    )
        .unwrap();
    contract
}

/// Deploy classhash and passes in world address to constructor
///
/// # Arguments
///
/// * `class_hash` - Class to deploy
/// * `world` - World dispatcher to pass as world address
///
/// # Returns
/// * address of contract deployed
pub fn deploy_with_world_address(class_hash: felt252, world: IWorldDispatcher) -> ContractAddress {
    deploy_contract(class_hash, [world.contract_address.into()].span())
}

/// Spawns a test world registering provided resources into namespaces.
///
/// # Arguments
///
/// * `namespaces_defs` - Definitions of namespaces to register.
///
/// # Returns
///
/// * World dispatcher
pub fn spawn_test_world(namespaces_defs: Span<NamespaceDef>) -> WorldStorage {
    let salt = core::testing::get_available_gas();

    let (world_address, _) = deploy_syscall(
        world::TEST_CLASS_HASH.try_into().unwrap(),
        salt.into(),
        [world::TEST_CLASS_HASH].span(),
        false
    )
        .unwrap();

    let world = IWorldDispatcher { contract_address: world_address };

    let mut first_namespace = Option::None;

    for ns in namespaces_defs {
        let namespace = ns.namespace.clone();
        world.register_namespace(namespace.clone());

        if first_namespace.is_none() {
            first_namespace = Option::Some(namespace.clone());
        }

        let namespace_hash = dojo::utils::bytearray_hash(@namespace);

        for r in ns
            .resources
            .clone() {
                match r {
                    TestResource::Event(ch) => { world.register_event(namespace.clone(), *ch); },
                    TestResource::Model(ch) => { world.register_model(namespace.clone(), *ch); },
                    TestResource::Contract(def) => {
                        let class_hash: ClassHash = (*def.class_hash).try_into().unwrap();
                        let contract_address = world
                            .register_contract(*def.class_hash, namespace.clone(), class_hash);

                        for target in *def
                            .writer_of {
                                world.grant_writer(*target, contract_address);
                            };

                        for target in *def
                            .owner_of {
                                world.grant_owner(*target, contract_address);
                            };

                        let selector = dojo::utils::selector_from_namespace_and_name(
                            namespace_hash, def.name
                        );
                        world.init_contract(selector, *def.init_calldata);
                    },
                }
            }
    };

    WorldStorageTrait::new(world, @first_namespace.unwrap())
}
