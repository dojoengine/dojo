use core::option::OptionTrait;
use core::traits::TryInto;

use starknet::ClassHash;

use dojo::contract::base;
use dojo::contract::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
use dojo::utils::test::{spawn_test_world};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};


#[starknet::contract]
pub mod contract_upgrade {
    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, IWorldProvider};

    #[storage]
    struct Storage {}

    #[starknet::interface]
    pub trait IQuantumLeap<TState> {
        fn plz_more_tps(self: @TState) -> felt252;
    }

    #[constructor]
    fn constructor(ref self: ContractState) {}

    #[abi(embed_v0)]
    pub impl QuantumLeap of IQuantumLeap<ContractState> {
        fn plz_more_tps(self: @ContractState) -> felt252 {
            'daddy'
        }
    }

    #[abi(embed_v0)]
    pub impl WorldProviderImpl of IWorldProvider<ContractState> {
        fn world(self: @ContractState) -> IWorldDispatcher {
            IWorldDispatcher { contract_address: starknet::contract_address_const::<'world'>() }
        }
    }
}

#[starknet::contract]
pub mod contract_invalid_upgrade {
    #[storage]
    struct Storage {}
}

use contract_upgrade::{IQuantumLeapDispatcher, IQuantumLeapDispatcherTrait};

// Utils
fn deploy_world() -> IWorldDispatcher {
    spawn_test_world(["dojo"].span(), [].span())
}

// A test contract needs to be used instead of previously used base contract since.
// contracts now require a `dojo_init` method which normal base contract doesn't have
#[dojo::contract]
mod test_contract {}

#[test]
#[available_gas(6000000)]
fn test_upgrade_from_world() {
    let world = deploy_world();

    let base_address = world
        .deploy_contract('salt', test_contract::TEST_CLASS_HASH.try_into().unwrap(),);
    let new_class_hash: ClassHash = contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    let selector = selector_from_tag!("dojo-test_contract");
    world.upgrade_contract(selector, new_class_hash);

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

    let _ = world.deploy_contract('salt', test_contract::TEST_CLASS_HASH.try_into().unwrap(),);
    let new_class_hash: ClassHash = contract_invalid_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    let selector = selector_from_tag!("dojo-test_contract");
    world.upgrade_contract(selector, new_class_hash);
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('must be called by world', 'ENTRYPOINT_FAILED'))]
fn test_upgrade_direct() {
    let world = deploy_world();

    let base_address = world
        .deploy_contract('salt', test_contract::TEST_CLASS_HASH.try_into().unwrap(),);
    let new_class_hash: ClassHash = contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address: base_address };
    upgradeable_dispatcher.upgrade(new_class_hash);
}

#[starknet::interface]
trait IMetadataOnly<T> {
    fn selector(self: @T) -> felt252;
    fn name(self: @T) -> ByteArray;
    fn namespace(self: @T) -> ByteArray;
    fn namespace_hash(self: @T) -> felt252;
}
