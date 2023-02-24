use array::ArrayTrait;
use starknet_serde::ContractAddressSerde;

#[abi]
trait IProxy {
    fn set_implementation(class_hash: felt);
    fn initialize(world_address: ContractAddress);
}

#[abi]
trait IWorld {
    fn lookup(from: felt) -> felt;
}

#[contract]
mod World {
    use hash::pedersen;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::contract_address_to_felt;
    use dojo::syscalls::deploy;

    struct Storage {
        entity_registry_len: LegacyMap::<felt, felt>,
        entity_registry: LegacyMap::<(felt, felt), felt>,
        module_registry: LegacyMap::<felt, felt>,
    }

    #[event]
    fn ComponentValueSet(component_id: felt, entity_id: felt, data: Array::<felt>) {}

    #[external]
    fn register(class_hash: felt, module_id: felt) {
        let module_id = pedersen(0, module_id);
        let proxy_class_hash = 420;
        let module_address = deploy(
            proxy_class_hash, module_id, ArrayTrait::new(), bool::False(())
        );
        let world_address = get_contract_address();
        super::IProxyDispatcher::set_implementation(module_address, class_hash);
        super::IProxyDispatcher::initialize(module_address, world_address);

        let module_address_felt = contract_address_to_felt(module_address);
        module_registry::write(module_id, module_address_felt);
        module_registry::write(module_address_felt, module_id);
    }

    #[external]
    fn on_component_set(entity_id: felt, data: Array::<felt>) {
        let caller_address = get_caller_address();
        let caller_address_felt = contract_address_to_felt(caller_address);
        assert(caller_address_felt != 0, 'World: not a registered');
        let entities_len = entity_registry_len::read(caller_address_felt);
        entity_registry::write((caller_address_felt, entities_len), entity_id);
        entity_registry_len::write(caller_address_felt, entities_len + 1);
        ComponentValueSet(caller_address_felt, entity_id, data);
    }

    #[view]
    fn lookup(from: felt) -> felt {
        module_registry::read(from)
    }
}
