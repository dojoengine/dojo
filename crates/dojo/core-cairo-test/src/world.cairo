use core::option::OptionTrait;
use core::result::ResultTrait;
use core::traits::{Into, TryInto};

use starknet::{ContractAddress, syscalls::deploy_syscall};

use dojo::world::{world, IWorldDispatcher, IWorldDispatcherTrait, WorldStorageTrait, WorldStorage};

pub type TestClassHash = felt252;

/// In Cairo test runner, all the classes are expected to be declared already.
/// If a contract belong to an other crate, it must be added to the `build-external-contract`,
/// event for testing, since Scarb does not do that automatically anymore.
///
/// The [`TestResource`] enum uses a felt252 to represent the class hash, this avoids
/// having to write `bar::TEST_CLASS_HASH.try_into().unwrap()` in the test file, simply use
/// `bar::TEST_CLASS_HASH`.
#[derive(Drop)]
pub enum TestResource {
    Event: TestClassHash,
    Model: TestClassHash,
    Contract: TestClassHash,
}

#[derive(Drop, Copy)]
pub enum ContractDescriptor {
    /// Address of the contract.
    Address: ContractAddress,
    /// Namespace and name of the contract.
    Named: (@ByteArray, @ByteArray),
}

/// Definition of a contract to register in the world.
///
/// You can use this struct for a dojo contract, but also for an external contract.
/// The only difference is the `init_calldata`, which is only used for dojo contracts.
/// If the `contract` is an external contract (hence an address), then `init_calldata` is ignored.
#[derive(Drop, Copy)]
pub struct ContractDef {
    /// The contract to grant permission to.
    pub contract: ContractDescriptor,
    /// Selectors of the resources that the contract is granted writer access to.
    pub writer_of: Span<felt252>,
    /// Selector of the resource that the contract is the owner of.
    pub owner_of: Span<felt252>,
    /// Calldata for dojo_init.
    pub init_calldata: Span<felt252>,
}

#[derive(Drop)]
pub struct NamespaceDef {
    pub namespace: ByteArray,
    pub resources: Span<TestResource>,
}

#[generate_trait]
pub impl ContractDefImpl of ContractDefTrait {
    fn new(namespace: @ByteArray, name: @ByteArray) -> ContractDef {
        ContractDef {
            contract: ContractDescriptor::Named((namespace, name)),
            writer_of: [].span(),
            owner_of: [].span(),
            init_calldata: [].span(),
        }
    }

    fn new_address(address: ContractAddress) -> ContractDef {
        ContractDef {
            contract: ContractDescriptor::Address(address),
            writer_of: [].span(),
            owner_of: [].span(),
            init_calldata: [].span(),
        }
    }

    fn with_init_calldata(mut self: ContractDef, init_calldata: Span<felt252>) -> ContractDef {
        match self.contract {
            ContractDescriptor::Address(_) => panic!(
                "Cannot set init_calldata for address descriptor",
            ),
            ContractDescriptor::Named(_) => self.init_calldata = init_calldata,
        };

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
        class_hash.try_into().unwrap(), 0, calldata, false,
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
/// This function only deploys the world and registers the resources, it does not initialize the
/// contracts or any permissions.
/// The first namespace is used as the default namespace when [`WorldStorage`] is returned.
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
        false,
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

        for r in ns.resources.clone() {
            match r {
                TestResource::Event(ch) => {
                    world.register_event(namespace.clone(), (*ch).try_into().unwrap());
                },
                TestResource::Model(ch) => {
                    world.register_model(namespace.clone(), (*ch).try_into().unwrap());
                },
                TestResource::Contract(ch) => {
                    world.register_contract(*ch, namespace.clone(), (*ch).try_into().unwrap());
                },
            }
        }
    };

    WorldStorageTrait::new(world, @first_namespace.unwrap())
}

#[generate_trait]
pub impl WorldStorageInternalTestImpl of WorldStorageTestTrait {
    fn sync_perms_and_inits(self: @WorldStorage, contracts: Span<ContractDef>) {
        // First, sync permissions as sozo is doing.
        for c in contracts {
            let contract_address = match c.contract {
                ContractDescriptor::Address(address) => *address,
                ContractDescriptor::Named((
                    namespace, name,
                )) => {
                    let selector = dojo::utils::selector_from_names(*namespace, *name);
                    match (*self.dispatcher).resource(selector) {
                        dojo::world::Resource::Contract((address, _)) => address,
                        _ => panic!("Contract not found"),
                    }
                },
            };

            for w in *c.writer_of {
                (*self.dispatcher).grant_writer(*w, contract_address);
            };

            for o in *c.owner_of {
                (*self.dispatcher).grant_owner(*o, contract_address);
            };
        };

        // Then, calls the dojo_init for each contract that is a dojo contract.
        for c in contracts {
            match c.contract {
                ContractDescriptor::Address(_) => {},
                ContractDescriptor::Named((
                    namespace, name,
                )) => {
                    let selector = dojo::utils::selector_from_names(*namespace, *name);
                    (*self.dispatcher).init_contract(selector, *c.init_calldata);
                },
            }
        };
    }
}
