use core::starknet::{ContractAddress, ClassHash};
use dojo::world::{world, IWorldDispatcherTrait};
use dojo::contract::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
use dojo::meta::{IDeployedResourceDispatcher, IDeployedResourceDispatcherTrait};
use crate::tests::helpers::{DOJO_NSH, test_contract, drop_all_events, deploy_world};

#[starknet::contract]
pub mod contract_invalid_upgrade {
    #[storage]
    struct Storage {}

    #[abi(per_item)]
    #[generate_trait]
    pub impl InvalidImpl of InvalidContractTrait {
        #[external(v0)]
        fn no_dojo_name(self: @ContractState) -> ByteArray {
            "test_contract"
        }
    }
}

#[starknet::interface]
pub trait IQuantumLeap<T> {
    fn plz_more_tps(self: @T) -> felt252;
}

#[starknet::contract]
pub mod test_contract_upgrade {
    use dojo::world::IWorldDispatcher;
    use dojo::contract::components::world_provider::IWorldProvider;

    #[storage]
    struct Storage {}

    #[constructor]
    fn constructor(ref self: ContractState) {}

    #[abi(embed_v0)]
    pub impl QuantumLeap of super::IQuantumLeap<ContractState> {
        fn plz_more_tps(self: @ContractState) -> felt252 {
            'daddy'
        }
    }

    #[abi(embed_v0)]
    pub impl WorldProviderImpl of IWorldProvider<ContractState> {
        fn world_dispatcher(self: @ContractState) -> IWorldDispatcher {
            IWorldDispatcher { contract_address: starknet::contract_address_const::<'world'>() }
        }
    }

    #[abi(embed_v0)]
    pub impl ContractImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "test_contract"
        }
    }
}

#[test]
#[available_gas(7000000)]
fn test_upgrade_from_world() {
    let world = deploy_world();
    let world = world.dispatcher;

    let base_address = world
        .register_contract('salt', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = test_contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    world.upgrade_contract("dojo", new_class_hash);

    let quantum_dispatcher = IQuantumLeapDispatcher { contract_address: base_address };
    assert(quantum_dispatcher.plz_more_tps() == 'daddy', 'quantum leap failed');
}

#[test]
#[available_gas(7000000)]
#[should_panic(expected: ('ENTRYPOINT_NOT_FOUND', 'ENTRYPOINT_FAILED', 'ENTRYPOINT_FAILED'))]
fn test_upgrade_from_world_not_world_provider() {
    let world = deploy_world();
    let world = world.dispatcher;

    let _ = world
        .register_contract('salt', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = contract_invalid_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    world.upgrade_contract("dojo", new_class_hash);
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('must be called by world', 'ENTRYPOINT_FAILED'))]
fn test_upgrade_direct() {
    let world = deploy_world();
    let world = world.dispatcher;

    let base_address = world
        .register_contract('salt', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = test_contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address: base_address };
    upgradeable_dispatcher.upgrade(new_class_hash);
}

#[starknet::interface]
trait IMetadataOnly<T> {
    fn dojo_name(self: @T) -> ByteArray;
}

#[starknet::contract]
mod invalid_legacy_model {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl InvalidModelMetadata of super::IMetadataOnly<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "invalid_legacy_model"
        }
    }
}

#[starknet::contract]
mod invalid_legacy_model_world {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl InvalidModelName of super::IMetadataOnly<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "invalid_legacy_model"
        }
    }
}

#[starknet::contract]
mod invalid_model {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl InvalidModelSelector of super::IMetadataOnly<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "invalid_model"
        }
    }
}

#[starknet::contract]
mod invalid_model_world {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl InvalidModelSelector of super::IMetadataOnly<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "invalid_model_world"
        }
    }
}

#[test]
fn test_deploy_contract_for_namespace_owner() {
    let world = deploy_world();
    let world = world.dispatcher;

    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();
    world.grant_owner(DOJO_NSH, bob);

    // the account owns the 'test_contract' namespace so it should be able to deploy the contract.
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    drop_all_events(world.contract_address);

    let contract_address = world.register_contract('salt1', "dojo", class_hash);

    let event = match starknet::testing::pop_log::<world::Event>(world.contract_address).unwrap() {
        world::Event::ContractRegistered(event) => event,
        _ => panic!("no ContractRegistered event"),
    };

    let contract = IDeployedResourceDispatcher { contract_address };
    let contract_name = contract.dojo_name();

    assert(event.name == contract_name, 'bad name');
    assert(event.namespace == "dojo", 'bad namespace');
    assert(event.salt == 'salt1', 'bad event salt');
    assert(event.class_hash == class_hash, 'bad class_hash');
    assert(
        event.address != core::num::traits::Zero::<ContractAddress>::zero(), 'bad contract address',
    );
}

#[test]
#[should_panic(
    expected: ("Account `0xb0b` does NOT have OWNER role on namespace `dojo`", 'ENTRYPOINT_FAILED'),
)]
fn test_deploy_contract_for_namespace_writer() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    world.grant_writer(DOJO_NSH, bob);

    // the account has write access to the 'test_contract' namespace so it should be able to deploy
    // the contract.
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: ("Account `0xb0b` does NOT have OWNER role on namespace `dojo`", 'ENTRYPOINT_FAILED'),
)]
fn test_deploy_contract_no_namespace_owner_access() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(expected: ("Namespace `buzz_namespace` is not registered", 'ENTRYPOINT_FAILED'))]
fn test_deploy_contract_with_unregistered_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    world
        .register_contract(
            'salt1', "buzz_namespace", test_contract::TEST_CLASS_HASH.try_into().unwrap(),
        );
}

#[test]
#[should_panic(
    expected: (
        "Contract `0xdead` does NOT have OWNER role on namespace `dojo`", 'ENTRYPOINT_FAILED',
    ),
)]
fn test_deploy_contract_through_malicious_contract() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(DOJO_NSH, bob);

    // the account owns the 'test_contract' namespace so it should be able to deploy the contract.
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(malicious_contract);

    world.register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
}
#[test]
fn test_upgrade_contract_from_resource_owner() {
    let world = deploy_world();
    let world = world.dispatcher;

    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_owner(DOJO_NSH, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let contract_address = world.register_contract('salt1', "dojo", class_hash);
    let contract = IDeployedResourceDispatcher { contract_address };
    let contract_name = contract.dojo_name();

    drop_all_events(world.contract_address);

    world.upgrade_contract("dojo", class_hash);

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::ContractUpgraded(event) = event.unwrap() {
        assert(
            event
                .selector == dojo::utils::selector_from_namespace_and_name(
                    DOJO_NSH, @contract_name,
                ),
            'bad contract selector',
        );
        assert(event.class_hash == class_hash, 'bad class_hash');
    } else {
        core::panic_with_felt252('no ContractUpgraded event');
    };
}

#[test]
#[should_panic(
    expected: (
        "Account `0xa11ce` does NOT have OWNER role on contract (or its namespace) `test_contract`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_contract_from_resource_writer() {
    let world = deploy_world();
    let world = world.dispatcher;

    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();
    let alice = starknet::contract_address_const::<0xa11ce>();

    world.grant_owner(DOJO_NSH, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let contract_address = world.register_contract('salt1', "dojo", class_hash);
    let contract = IDeployedResourceDispatcher { contract_address };
    let contract_name = contract.dojo_name();
    let contract_selector = dojo::utils::selector_from_namespace_and_name(DOJO_NSH, @contract_name);

    world.grant_writer(contract_selector, alice);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.upgrade_contract("dojo", class_hash);
}

#[test]
#[should_panic(
    expected: (
        "Account `0xa11ce` does NOT have OWNER role on contract (or its namespace) `test_contract`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_contract_from_random_account() {
    let world = deploy_world();
    let world = world.dispatcher;

    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let _contract_address = world.register_contract('salt1', "dojo", class_hash);

    let alice = starknet::contract_address_const::<0xa11ce>();

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.upgrade_contract("dojo", class_hash);
}

#[test]
#[should_panic(
    expected: (
        "Contract `0xdead` does NOT have OWNER role on contract (or its namespace) `test_contract`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_contract_through_malicious_contract() {
    let world = deploy_world();
    let world = world.dispatcher;

    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(DOJO_NSH, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let _contract_address = world.register_contract('salt1', "dojo", class_hash);

    starknet::testing::set_contract_address(malicious_contract);

    world.upgrade_contract("dojo", class_hash);
}
