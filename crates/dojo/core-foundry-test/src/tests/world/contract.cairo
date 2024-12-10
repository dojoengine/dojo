use dojo::world::{world, IWorldDispatcherTrait};
use dojo::contract::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
use dojo::meta::{IDeployedResourceDispatcher, IDeployedResourceDispatcherTrait};
use crate::tests::helpers::{DOJO_NSH, deploy_world};
use crate::snf_utils;

use snforge_std::{spy_events, EventSpyAssertionsTrait};

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
        .register_contract('salt', "dojo", snf_utils::declare_contract("test_contract"));

    world.upgrade_contract("dojo", snf_utils::declare_contract("test_contract_upgrade"));

    let quantum_dispatcher = IQuantumLeapDispatcher { contract_address: base_address };
    assert(quantum_dispatcher.plz_more_tps() == 'daddy', 'quantum leap failed');
}

#[test]
#[available_gas(7000000)]
#[should_panic(expected: ('ENTRYPOINT_NOT_FOUND', 'ENTRYPOINT_FAILED'))]
fn test_upgrade_from_world_not_world_provider() {
    let world = deploy_world();
    let world = world.dispatcher;

    let _ = world.register_contract('salt', "dojo", snf_utils::declare_contract("test_contract"));
    world.upgrade_contract("dojo", snf_utils::declare_contract("contract_invalid_upgrade"));
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: 'must be called by world')]
fn test_upgrade_direct() {
    let world = deploy_world();
    let world = world.dispatcher;

    let base_address = world
        .register_contract('salt', "dojo", snf_utils::declare_contract("test_contract"));

    let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address: base_address };
    upgradeable_dispatcher.upgrade(snf_utils::declare_contract("test_contract_upgrade"));
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

    let class_hash = snf_utils::declare_contract("test_contract");

    let bob = starknet::contract_address_const::<0xb0b>();
    world.grant_owner(DOJO_NSH, bob);

    // the account owns the 'test_contract' namespace so it should be able to deploy the contract.
    snf_utils::set_account_address(bob);
    snf_utils::set_caller_address(bob);

    let mut spy = spy_events();

    let contract_address = world.register_contract('salt1', "dojo", class_hash);

    spy
        .assert_emitted(
            @array![
                (
                    world.contract_address,
                    world::Event::ContractRegistered(
                        world::ContractRegistered {
                            name: "test_contract",
                            namespace: "dojo",
                            address: contract_address,
                            class_hash: class_hash,
                            salt: 'salt1'
                        }
                    )
                )
            ]
        );
}

#[test]
#[should_panic(expected: "Account `2827` does NOT have OWNER role on namespace `dojo`")]
fn test_deploy_contract_for_namespace_writer() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    world.grant_writer(DOJO_NSH, bob);

    // the account has write access to the 'test_contract' namespace so it should be able to deploy
    // the contract.
    snf_utils::set_account_address(bob);
    snf_utils::set_caller_address(bob);

    world.register_contract('salt1', "dojo", snf_utils::declare_contract("test_contract"));
}

#[test]
#[should_panic(expected: "Account `2827` does NOT have OWNER role on namespace `dojo`")]
fn test_deploy_contract_no_namespace_owner_access() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    snf_utils::set_account_address(bob);
    snf_utils::set_caller_address(bob);

    world.register_contract('salt1', "dojo", snf_utils::declare_contract("test_contract"));
}

#[test]
#[should_panic(expected: "Namespace `buzz_namespace` is not registered")]
fn test_deploy_contract_with_unregistered_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    world
        .register_contract('salt1', "buzz_namespace", snf_utils::declare_contract("test_contract"));
}

// It's ENTRYPOINT_NOT_FOUND for now as in this example the contract is not a dojo contract
// and it's not the account that is calling the deploy_contract function.
#[test]
#[should_panic(expected: ('ENTRYPOINT_NOT_FOUND', 'ENTRYPOINT_FAILED'))]
fn test_deploy_contract_through_malicious_contract() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = snf_utils::declare_and_deploy("malicious_contract");

    world.grant_owner(DOJO_NSH, bob);

    // the account owns the 'test_contract' namespace so it should be able to deploy the contract.
    snf_utils::set_account_address(bob);
    snf_utils::set_caller_address(malicious_contract);

    world.register_contract('salt1', "dojo", snf_utils::declare_contract("test_contract"));
}
#[test]
fn test_upgrade_contract_from_resource_owner() {
    let world = deploy_world();
    let world = world.dispatcher;

    let class_hash = snf_utils::declare_contract("test_contract");

    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_owner(DOJO_NSH, bob);

    snf_utils::set_account_address(bob);
    snf_utils::set_caller_address(bob);

    let _ = world.register_contract('salt1', "dojo", class_hash);

    let mut spy = spy_events();

    world.upgrade_contract("dojo", class_hash);

    spy
        .assert_emitted(
            @array![
                (
                    world.contract_address,
                    world::Event::ContractUpgraded(
                        world::ContractUpgraded {
                            selector: dojo::utils::selector_from_namespace_and_name(
                                DOJO_NSH, @"test_contract"
                            ),
                            class_hash: class_hash,
                        }
                    )
                )
            ]
        );
}

#[test]
#[should_panic(
    expected: "Account `659918` does NOT have OWNER role on contract (or its namespace) `test_contract`"
)]
fn test_upgrade_contract_from_resource_writer() {
    let world = deploy_world();
    let world = world.dispatcher;

    let class_hash = snf_utils::declare_contract("test_contract");

    let bob = starknet::contract_address_const::<0xb0b>();
    let alice = starknet::contract_address_const::<0xa11ce>();

    world.grant_owner(DOJO_NSH, bob);

    snf_utils::set_account_address(bob);
    snf_utils::set_caller_address(bob);

    let contract_address = world.register_contract('salt1', "dojo", class_hash);
    let contract = IDeployedResourceDispatcher { contract_address };
    let contract_name = contract.dojo_name();
    let contract_selector = dojo::utils::selector_from_namespace_and_name(DOJO_NSH, @contract_name);

    world.grant_writer(contract_selector, alice);

    snf_utils::set_account_address(alice);
    snf_utils::set_caller_address(alice);

    world.upgrade_contract("dojo", class_hash);
}

#[test]
#[should_panic(
    expected: "Account `659918` does NOT have OWNER role on contract (or its namespace) `test_contract`"
)]
fn test_upgrade_contract_from_random_account() {
    let world = deploy_world();
    let world = world.dispatcher;

    let class_hash = snf_utils::declare_contract("test_contract");

    let _contract_address = world.register_contract('salt1', "dojo", class_hash);

    let alice = starknet::contract_address_const::<0xa11ce>();

    snf_utils::set_account_address(alice);
    snf_utils::set_caller_address(alice);

    world.upgrade_contract("dojo", class_hash);
}

#[test]
#[should_panic(expected: ('ENTRYPOINT_NOT_FOUND', 'ENTRYPOINT_FAILED'))]
fn test_upgrade_contract_through_malicious_contract() {
    let world = deploy_world();
    let world = world.dispatcher;

    let class_hash = snf_utils::declare_contract("test_contract");

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = snf_utils::declare_and_deploy("malicious_contract");

    world.grant_owner(DOJO_NSH, bob);

    snf_utils::set_account_address(bob);
    snf_utils::set_caller_address(bob);

    let _contract_address = world.register_contract('salt1', "dojo", class_hash);

    snf_utils::set_caller_address(malicious_contract);

    world.upgrade_contract("dojo", class_hash);
}
