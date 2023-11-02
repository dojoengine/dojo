use option::OptionTrait;
use starknet::ClassHash;
use traits::TryInto;

use dojo::base::base;
use dojo::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
use dojo::test_utils::deploy_contract;

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

// TODO : rewrite & use world
// #[test]
// #[available_gas(6000000)]
// fn test_upgrade() {
//     let base_address = deploy_contract(base::TEST_CLASS_HASH, array![].span());
//     let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address: base_address };

//     let new_class_hash: ClassHash = contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();
//     upgradeable_dispatcher.upgrade(new_class_hash);

//     let quantum_dispatcher = IQuantumLeapDispatcher { contract_address: base_address };
//     assert(quantum_dispatcher.plz_more_tps() == 'daddy', 'quantum leap failed');
// }
