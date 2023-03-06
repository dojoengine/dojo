use array::ArrayTrait;
use starknet::contract_address::ContractAddressSerde;

#[abi]
trait IProxy {
    fn set_implementation(class_hash: felt);
    fn initialize(world_address: ContractAddress);
}

#[abi]
trait IWorld {
    fn entities(component: starknet::ContractAddress) -> felt;
}

#[contract]
mod World {
    use array::ArrayTrait;
    use hash::pedersen;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::contract_address_to_felt;
    use starknet::ContractAddressZeroable;
    use dojo::hash::LegacyHashContractAddressUsizePair;
    use dojo::serde::ArrayU32Serde;
    use dojo::syscalls::deploy;
    use super::IProxyDispatcher;
    use super::IProxyDispatcherTrait;

    struct Storage {
        entity_registry_len: LegacyMap::<starknet::ContractAddress, usize>,
        entity_registry: LegacyMap::<(starknet::ContractAddress, usize), usize>,
        module_registry: LegacyMap::<starknet::ContractAddress, bool>,
    }

    #[event]
    fn ComponentValueSet(
        component_address: starknet::ContractAddress, entity_id: usize, data: Array::<felt>
    ) {}

    #[event]
    fn ModuleRegistered(
        module_address: starknet::ContractAddress, module_id: felt, class_hash: felt
    ) {}

    #[external]
    fn register(class_hash: felt, module_id: felt) {
        let module_id = pedersen(0, module_id);
        let proxy_class_hash = 420;
        let module_address = deploy(
            proxy_class_hash, module_id, ArrayTrait::new(), bool::False(())
        );
        let world_address = get_contract_address();
        IProxyDispatcher { contract_address: module_address }.set_implementation(class_hash);
        IProxyDispatcher { contract_address: module_address }.initialize(world_address);
        module_registry::write(module_address, bool::True(()));
        ModuleRegistered(module_address, module_id, class_hash);
    }

    #[external]
    fn on_component_set(entity_id: usize, data: Array::<felt>) {
        let caller_address = get_caller_address();
        assert(module_registry::read(caller_address), 'component not a registered');
        let entities_len = entity_registry_len::read(caller_address);
        entity_registry::write((caller_address, entities_len), entity_id);
        entity_registry_len::write(caller_address, entities_len + 1_usize);
        ComponentValueSet(caller_address, entity_id, data);
    }

    fn get_entities_inner(
        component_address: starknet::ContractAddress,
        entities_len: usize,
        ref entities: Array::<usize>
    ) {
        match gas::get_gas() {
            Option::Some(_) => {},
            Option::None(_) => {
                let mut data = ArrayTrait::new();
                data.append('OOG');
                panic(data);
            }
        }

        if (entities_len == 0_usize) {
            return ();
        }

        let entity_id = entity_registry::read((component_address, (entities_len - 1_usize)));
        entities.append(entity_id);
        return get_entities_inner(component_address, entities_len - 1_usize, ref entities);
    }

    #[view]
    fn get_entities(component_address: starknet::ContractAddress) -> Array::<usize> {
        let entities_len = entity_registry_len::read(component_address);
        let mut entities = array_new::<u32>();
        get_entities_inner(component_address, entities_len, ref entities);
        return entities;
    }
}
// #[test]
// #[available_gas(2000000)]
// fn on_component_set() {
//     World::register(420, 69);
//     starknet_testing::set_caller_address(starknet::contract_address_const::<0x420>());
//     let data = ArrayTrait::new();
//     World::on_component_set(69_usize, data);
// }


