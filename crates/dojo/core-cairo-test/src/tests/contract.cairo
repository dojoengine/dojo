use core::option::OptionTrait;
use core::traits::TryInto;

use starknet::ClassHash;

use dojo::contract::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
use dojo::world::IWorldDispatcherTrait;

use crate::world::spawn_test_world;
use crate::tests::helpers::deploy_world;

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

#[dojo::contract]
mod test_contract {}

#[starknet::interface]
pub trait IQuantumLeap<T> {
    fn plz_more_tps(self: @T) -> felt252;
}

#[starknet::contract]
pub mod test_contract_upgrade {
    use dojo::contract::IContract;
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
    pub impl ContractImpl of IContract<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "test_contract"
        }
    }
}

#[test]
#[available_gas(7000000)]
fn test_upgrade_from_world() {
    let world = deploy_world();

    let base_address = world
        .register_contract('salt', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = test_contract_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    world.upgrade_contract("dojo", new_class_hash);

    let quantum_dispatcher = IQuantumLeapDispatcher { contract_address: base_address };
    assert(quantum_dispatcher.plz_more_tps() == 'daddy', 'quantum leap failed');
}

#[test]
#[available_gas(7000000)]
#[should_panic(
    expected: ('ENTRYPOINT_NOT_FOUND', 'ENTRYPOINT_FAILED')
)]
fn test_upgrade_from_world_not_world_provider() {
    let world = deploy_world();

    let _ = world.register_contract('salt', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
    let new_class_hash: ClassHash = contract_invalid_upgrade::TEST_CLASS_HASH.try_into().unwrap();

    world.upgrade_contract("dojo", new_class_hash);
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('must be called by world', 'ENTRYPOINT_FAILED'))]
fn test_upgrade_direct() {
    let world = deploy_world();

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
#[available_gas(6000000)]
#[should_panic(expected: ("Namespace `` is invalid according to Dojo naming rules: ^[a-zA-Z0-9_]+$", 'ENTRYPOINT_FAILED',))]
fn test_register_namespace_empty_name() {
    let world = deploy_world();

    world.register_namespace("");
}
