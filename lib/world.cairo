use array::ArrayTrait;

#[abi]
trait IProxy {
    fn set_implementation(class_hash: felt);
    fn initialize(world_address: felt);
}

#[contract]
mod World {
    use hash::pedersen;
    use starknet::get_caller_address;
    use syscalls::deploy;
    use syscalls::get_contract_address;

    struct Storage {
        entity_registry: LegacyMap::<felt, Array::<felt>>,
        module_registry: LegacyMap::<felt, felt>,
    }

    #[event]
    fn ComponentValueSet(component_id: felt, entity_id: felt, data: Array::<felt>) {}

    #[external]
    fn register(class_hash: felt, module_id: felt) {
        let module_id = pedersen(0, module_id);
        let address = deploy(
            class_hash = proxy_class_hash,
            contract_address_salt = module_id,
            constructor_calldata = Array::<felt>::new(),
            deploy_from_zero = bool::False(()),
        );

        let world_address = get_contract_address();
        super::IProxyDispatcher::set_implementation(address, class_hash);
        super::IProxyDispatcher::initialize(address, world_address);

        module_registry::write(module_id, address);
        module_registry::write(address, module_id);
    }

    #[external]
    fn on_component_set(entity_id: felt, data: Array::<felt>) {
        let caller_address = get_caller_address();
        assert(caller_address != 0, 'World: caller is not a registered');
        ComponentValueSet(caller_address, entity_id, data);
        let mut entities = entity_registry::read(caller_address);
        array_append(ref entities, entity_id);
        entity_registry::write(caller_address, entities);
    }

    #[view]
    fn lookup(from: felt) -> felt {
        module_registry::read(from)
    }
}
