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
    spawn_test_world("dojo", array![])
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
        .deploy_contract(
            'salt', test_contract::TEST_CLASS_HASH.try_into().unwrap(), array![].span()
        );
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

    let base_address = world
        .deploy_contract(
            'salt', test_contract::TEST_CLASS_HASH.try_into().unwrap(), array![].span()
        );
    let new_class_hash: ClassHash = contract_invalid_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    world.upgrade_contract(base_address, new_class_hash);
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('must be called by world', 'ENTRYPOINT_FAILED'))]
fn test_upgrade_direct() {
    let world = deploy_world();

    let base_address = world
        .deploy_contract(
            'salt', test_contract::TEST_CLASS_HASH.try_into().unwrap(), array![].span()
        );
    let new_class_hash: ClassHash = contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    let upgradeable_dispatcher = IUpgradeableDispatcher { contract_address: base_address };
    upgradeable_dispatcher.upgrade(new_class_hash);
}

#[starknet::interface]
trait IMetadataOnly<T> {
    fn selector(self: @T) -> felt252;
    fn name(self: @T) -> ByteArray;
    fn namespace(self: @T) -> ByteArray;
    fn namespace_selector(self: @T) -> felt252;
}

#[starknet::contract]
mod invalid_legacy_model {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl InvalidModelMetadata of super::IMetadataOnly<ContractState> {
        fn selector(self: @ContractState) -> felt252 {
            // Pre-computed address of a contract deployed through the world.
            0x1b1edb46931b1a98d8c6ecf2703e8483ec1d85fb75b3e9c061eab383fc8f8f1
        }

        fn namespace(self: @ContractState) -> ByteArray {
            "dojo"
        }

        fn namespace_selector(self: @ContractState) -> felt252 {
            dojo::utils::hash(@Self::namespace(self))
        }

        fn name(self: @ContractState) -> ByteArray {
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
        fn selector(self: @ContractState) -> felt252 {
            // World address is 0, and not registered as deployed through the world
            // as it's itself.
            0
        }

        fn namespace(self: @ContractState) -> ByteArray {
            "dojo"
        }

        fn namespace_selector(self: @ContractState) -> felt252 {
            dojo::utils::hash(@Self::namespace(self))
        }

        fn name(self: @ContractState) -> ByteArray {
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
        fn selector(self: @ContractState) -> felt252 {
            // NOTE: Need to update this value if address changes
            // Pre-computed address of a contract deployed through the world.
            0x314a23ab2b297b235fe87bf5acade82bcef62e3a375183ab7bf1fe0a3f5e8dd
        }

        fn namespace(self: @ContractState) -> ByteArray {
            "dojo"
        }

        fn namespace_selector(self: @ContractState) -> felt252 {
            dojo::utils::hash(@Self::namespace(self))
        }

        fn name(self: @ContractState) -> ByteArray {
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
        fn selector(self: @ContractState) -> felt252 {
            // World address is 0, and not registered as deployed through the world
            // as it's itself.
            0
        }

        fn namespace(self: @ContractState) -> ByteArray {
            "dojo"
        }

        fn namespace_selector(self: @ContractState) -> felt252 {
            dojo::utils::hash(@Self::namespace(self))
        }

        fn name(self: @ContractState) -> ByteArray {
            "invalid_model_world"
        }
    }
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('invalid model name', 'ENTRYPOINT_FAILED',))]
fn test_deploy_from_world_invalid_model() {
    let world = deploy_world();

    let contract_address = world
        .deploy_contract(0, test_contract::TEST_CLASS_HASH.try_into().unwrap(), array![].span());

    // This print allows to know the address of the deployed contract which must be returned
    // by the selector() function of invalid model, to simulate a ACL issue
    // (see register_model function)
    contract_address.print();

    world.register_model(invalid_model::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('invalid model name', 'ENTRYPOINT_FAILED',))]
fn test_deploy_from_world_invalid_model_world() {
    let world = deploy_world();
    world.register_model(invalid_model_world::TEST_CLASS_HASH.try_into().unwrap());
}
