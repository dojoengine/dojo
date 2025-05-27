use dojo::contract::components::upgradeable::{IUpgradeableDispatcher, IUpgradeableDispatcherTrait};
use dojo::world::IWorldDispatcherTrait;
use dojo_snf_test;
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
    use dojo::contract::components::world_provider::IWorldProvider;
    use dojo::meta::IDeployedResource;
    use dojo::world::IWorldDispatcher;

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
            IWorldDispatcher { contract_address: 'world'.try_into().unwrap() }
        }
    }

    #[abi(embed_v0)]
    pub impl ContractImpl of IContract<ContractState> {}

    #[abi(embed_v0)]
    pub impl Contract_DeployedContractImpl of IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "test_contract"
        }
    }
}

#[test]
#[available_gas(l2_gas: 7000000)]
fn test_upgrade_from_world() {
    let world = deploy_world();
    let world = world.dispatcher;

    let base_address = world
        .register_contract('salt', "dojo", dojo_snf_test::declare_contract("test_contract"));
    let new_class_hash = dojo_snf_test::declare_contract("test_contract_upgrade");

    world.upgrade_contract("dojo", new_class_hash);

    let quantum_dispatcher = IQuantumLeapDispatcher { contract_address: base_address };
    assert(quantum_dispatcher.plz_more_tps() == 'daddy', 'quantum leap failed');
}

#[test]
#[available_gas(l2_gas: 7000000)]
#[should_panic(expected: 'ENTRYPOINT_NOT_FOUND')]
fn test_upgrade_from_world_not_world_provider() {
    let world = deploy_world();
    let world = world.dispatcher;

    let _ = world
        .register_contract('salt', "dojo", dojo_snf_test::declare_contract("test_contract"));
    let new_class_hash = dojo_snf_test::declare_contract("contract_invalid_upgrade");

    world.upgrade_contract("dojo", new_class_hash);
}

#[test]
#[available_gas(l2_gas: 6000000)]
#[should_panic(expected: ('must be called by world',))]
fn test_upgrade_direct() {
    let world = deploy_world();
    let world = world.dispatcher;

    let base_address = world
        .register_contract('salt', "dojo", dojo_snf_test::declare_contract("test_contract"));
    let new_class_hash = dojo_snf_test::declare_contract("test_contract_upgrade");

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
#[available_gas(l2_gas: 6000000)]
#[should_panic(expected: "Namespace `` is invalid according to Dojo naming rules: ^[a-zA-Z0-9_]+$")]
fn test_register_namespace_empty_name() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_namespace("");
}
