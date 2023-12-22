use option::OptionTrait;
use starknet::ClassHash;
use traits::TryInto;

use dojo::base::base;
use dojo::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
use dojo::test_utils::{spawn_test_world};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};


#[starknet::contract]
mod contract_upgrade {
    #[storage]
    struct Storage {}

    #[starknet::interface]
    trait IQuantumLeap<TState> {
        fn plz_more_tps(self: @TState) -> felt252;
    }

    #[constructor]
    fn constructor(ref self: ContractState) {}

    #[external(v0)]
    impl QuantumLeap of IQuantumLeap<ContractState> {
        fn plz_more_tps(self: @ContractState) -> felt252 {
            'daddy'
        }
    }
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
#[should_panic(expected: ('must be called by world', 'ENTRYPOINT_FAILED'))]
fn test_upgrade_direct() {
    let world = deploy_world();

    let base_address = world.deploy_contract('salt', base::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address: base_address };
    upgradeable_dispatcher.upgrade(new_class_hash);
}
