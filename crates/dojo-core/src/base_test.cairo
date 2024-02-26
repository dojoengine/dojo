use debug::PrintTrait;
use option::OptionTrait;
use starknet::ClassHash;
use traits::TryInto;

use dojo::base::base;
use dojo::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
use dojo::test_utils::{spawn_test_world};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};


#[starknet::contract]
mod contract_upgrade {
    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, IWorldProvider};

    #[storage]
    struct Storage {}

    #[starknet::interface]
    trait IQuantumLeap<TState> {
        fn plz_more_tps(self: @TState) -> felt252;
    }

    #[constructor]
    fn constructor(ref self: ContractState) {}

    #[abi(embed_v0)]
    impl QuantumLeap of IQuantumLeap<ContractState> {
        fn plz_more_tps(self: @ContractState) -> felt252 {
            'daddy'
        }
    }

    #[abi(embed_v0)]
    impl WorldProviderImpl of IWorldProvider<ContractState> {
        fn world(self: @ContractState) -> IWorldDispatcher {
            IWorldDispatcher { contract_address: starknet::contract_address_const::<'world'>() }
        }
    }
}

#[starknet::contract]
mod contract_invalid_upgrade {
    #[storage]
    struct Storage {}
}

use contract_upgrade::{IQuantumLeapDispatcher, IQuantumLeapDispatcherTrait};

// Utils
fn deploy_world() -> IWorldDispatcher {
    spawn_test_world(array![])
}

#[test]
#[available_gas(6000000)]
fn test_upgrade_from_world() {
    let world = deploy_world();

    let base_address = world.deploy_contract('salt', base::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    world.upgrade_contract(base_address, new_class_hash);

    let quantum_dispatcher = IQuantumLeapDispatcher { contract_address: base_address };
    assert(quantum_dispatcher.plz_more_tps() == 'daddy', 'quantum leap failed');
}

#[test]
#[available_gas(6000000)]
#[should_panic(
    expected: ('class_hash not world provider', 'ENTRYPOINT_FAILED', 'ENTRYPOINT_FAILED')
)]
fn test_upgrade_from_world_not_world_provider() {
    let world = deploy_world();

    let base_address = world.deploy_contract('salt', base::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = contract_invalid_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    world.upgrade_contract(base_address, new_class_hash);
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('must be called by world', 'ENTRYPOINT_FAILED'))]
fn test_upgrade_direct() {
    let world = deploy_world();

    let base_address = world.deploy_contract('salt', base::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address: base_address };
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
            0x742c3d09472a40914dedcbd609788fd547bde613d6c4d4c2f15d41f4e241f25
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

    let base_address = world.deploy_contract(0, base::TEST_CLASS_HASH.try_into().unwrap());
    // The print is required for invalid_model name to be a valid address as the
    // register_model will use the gas consumed as salt.
    base_address.print();

    world.register_model(invalid_model::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('invalid model name', 'ENTRYPOINT_FAILED',))]
fn test_deploy_from_world_invalid_model_world() {
    let world = deploy_world();
    world.register_model(invalid_model_world::TEST_CLASS_HASH.try_into().unwrap());
}
