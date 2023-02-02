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
        registry: LegacyMap::<felt, felt>, 
    }

    #[event]
    fn ComponentValueSet(component_id: felt, entity_id: felt, data: Array::<T>) {}

    #[external]
    fn register(class_hash: felt, module_id: felt) {
        let module_id = pedersen(module_id);
        let address = deploy(
            class_hash = proxy_class_hash,
            contract_address_salt = module_id,
            constructor_calldata = Array::<felt>::new(),
            deploy_from_zero = bool::False(()),
        );

        let world_address = get_contract_address();
        super::IProxyDispatcher::set_implementation(address, class_hash);
        super::IproxyDispatcher::initialize(address, world_address);

        registry.write(module_id, address);
        registry.write(address, module_id);
    }

    #[external]
    fn on_component_set(entity_id: felt, data: Array::<T>) {
        let caller_address = get_caller_address();
        let module_id = registry.read(caller_address);
        assert(module_id != 0, 'World: caller is not a registered');
        ComponentValueSet(caller_address, enitiy_id, data);
    }

    #[view]
    fn lookup(from: felt) -> felt {
        registry.read(from)
    }
}
