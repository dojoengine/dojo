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
    use dojo::syscalls::deploy;
    use dojo::syscalls::get_contract_address;

    struct Storage {
        entity_registry_len: LegacyMap::<felt, felt>,
        entity_registry: LegacyMap::<(felt, felt), felt>,
        module_registry: LegacyMap::<felt, felt>,
    }

    // TODO: Uncommenting this gives:
    // error: Variable was previously moved.
    // --> contract:131:9
    //         serde::Serde::<felt>::serialize(ref data, entity_id);
    // #[event]
    // fn ComponentValueSet(component_id: felt, entity_id: felt, data: Array::<felt>) {}

    #[external]
    fn register(class_hash: felt, module_id: felt) {
        let module_id = pedersen(0, module_id);
        let proxy_class_hash = 420;
        let address = deploy(proxy_class_hash, module_id, ArrayTrait::new(), bool::False(()));

        match starknet::contract_address_try_from_felt(address) {
            Option::Some(ca) => {
                let world_address = get_contract_address();
                super::IProxyDispatcher::set_implementation(ca, class_hash);
                super::IProxyDispatcher::initialize(ca, world_address);

                module_registry::write(module_id, address);
                module_registry::write(address, module_id);
            },
            Option::None(_) => {},
        };
    }

    #[external]
    fn on_component_set(entity_id: felt, data: Array::<felt>) {
        let caller_address = get_caller_address();
        assert(caller_address != 0, 'World: caller is not a registered');
        // ComponentValueSet(caller_address, entity_id, data);
        let entities_len = entity_registry_len::read(caller_address);
        entity_registry::write((caller_address, entities_len), entity_id);
        entity_registry_len::write(caller_address, entities_len + 1);
    }

    #[view]
    fn lookup(from: felt) -> felt {
        module_registry::read(from)
    }
}
