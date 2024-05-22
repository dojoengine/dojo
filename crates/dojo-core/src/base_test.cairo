use debug::PrintTrait;
use option::OptionTrait;
use starknet::ClassHash;
use traits::TryInto;

use dojo::base::base;
use dojo::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
use dojo::test_utils::{spawn_test_world};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

mod v0 {
    #[dojo::contract]
    mod contract_upgrade {
        #[starknet::interface]
        trait IQuantumLeap<TState> {
            fn plz_more_tps(self: @TState) -> felt252;
        }

        #[abi(embed_v0)]
        impl QuantumLeap of IQuantumLeap<ContractState> {
            fn plz_more_tps(self: @ContractState) -> felt252 {
                'no its v0'
            }
        }
    }
}

mod v1 {
    #[dojo::contract]
    mod contract_upgrade {
        #[starknet::interface]
        trait IQuantumLeap<TState> {
            fn plz_more_tps(self: @TState) -> felt252;
        }

        #[abi(embed_v0)]
        impl QuantumLeap of IQuantumLeap<ContractState> {
            fn plz_more_tps(self: @ContractState) -> felt252 {
                'daddy'
            }
        }
    }
}

#[starknet::contract]
mod contract_not_world_provider {
    #[storage]
    struct Storage {}
}

use v1::contract_upgrade::{IQuantumLeapDispatcher, IQuantumLeapDispatcherTrait};

// Utils
fn deploy_world() -> IWorldDispatcher {
    spawn_test_world(array![])
}

#[test]
#[available_gas(6000000)]
fn test_upgrade_from_world() {
    let world = deploy_world();

    let v0_address = world
        .deploy_contract('salt', v0::contract_upgrade::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = v1::contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    world.upgrade_contract(v0_address, new_class_hash);

    let quantum_dispatcher = IQuantumLeapDispatcher { contract_address: v0_address };
    assert(quantum_dispatcher.plz_more_tps() == 'daddy', 'quantum leap failed');
}

#[test]
#[available_gas(6000000)]
#[should_panic(
    expected: ('class_hash not world provider', 'ENTRYPOINT_FAILED', 'ENTRYPOINT_FAILED')
)]
fn test_upgrade_from_world_not_world_provider() {
    let world = deploy_world();

    let v0_address = world
        .deploy_contract('salt', v0::contract_upgrade::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = contract_not_world_provider::TEST_CLASS_HASH.try_into().unwrap();

    world.upgrade_contract(v0_address, new_class_hash);
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('must be called by world', 'ENTRYPOINT_FAILED'))]
fn test_upgrade_direct() {
    let world = deploy_world();

    let v0_address = world
        .deploy_contract('salt', v0::contract_upgrade::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = v1::contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address: v0_address };
    upgradeable_dispatcher.upgrade(new_class_hash);
}

#[starknet::interface]
trait INameOnly<T> {
    fn name(self: @T) -> felt252;
}

#[starknet::contract]
mod invalid_model {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl InvalidModelName of super::INameOnly<ContractState> {
        fn name(self: @ContractState) -> felt252 {
            // Pre-computed address of a contract deployed through the world.
            // To print this addres, run:
            // sozo test --manifest-path crates/dojo-core/Scarb.toml -f test_deploy_from_world_invalid_model
            0x5c5e3c5bfe74b54f17d81863851b6f87fc9684828e61e6ce2894fb885b51ff1
        }
    }
}

#[starknet::contract]
mod invalid_model_world {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl InvalidModelName of super::INameOnly<ContractState> {
        fn name(self: @ContractState) -> felt252 {
            // World address is 0, and not registered as deployed through the world
            // as it's itself.
            0
        }
    }
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('invalid model name', 'ENTRYPOINT_FAILED',))]
fn test_deploy_from_world_invalid_model() {
    let world = deploy_world();

    let v0_address = world
        .deploy_contract(0, v0::contract_upgrade::TEST_CLASS_HASH.try_into().unwrap());
    // The print is required for invalid_model name to be a valid address as the
    // register_model will use the gas consumed as salt.
    v0_address.print();

    world.register_model(invalid_model::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('invalid model name', 'ENTRYPOINT_FAILED',))]
fn test_deploy_from_world_invalid_model_world() {
    let world = deploy_world();
    world.register_model(invalid_model_world::TEST_CLASS_HASH.try_into().unwrap());
}
