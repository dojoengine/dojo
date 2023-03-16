use array::ArrayTrait;
use hash::LegacyHash;
use starknet::contract_address::ContractAddressSerde;

#[abi]
trait IProxy {
    fn set_implementation(class_hash: felt252);
    fn initialize(world_address: starknet::ContractAddress);
}

#[abi]
trait IWorld {
    fn next_entity_id(path: (felt252, felt252, felt252)) -> (felt252, felt252, felt252, felt252);
    fn owner_of(entity_id: (felt252, felt252, felt252, felt252)) -> starknet::ContractAddress;
    fn entities(component: starknet::ContractAddress) -> Array<(felt252, felt252, felt252, felt252)>;
}

trait ComponentTrait<T> {
    fn initialize();
    fn set(entity_id: (felt252, felt252, felt252, felt252), value: T);
    fn get(entity_id: (felt252, felt252, felt252, felt252)) -> T;
}

impl LegacyHashEntityRegsiteryTuple of LegacyHash::<(starknet::ContractAddress, felt252, felt252, felt252, felt252)> {
    fn hash(state: felt252, tuple: (starknet::ContractAddress, felt252, felt252, felt252, felt252)) -> felt252 {
        let (first, second, third, fourth, fifth) = tuple;
        let state = LegacyHash::hash(state, first);
        LegacyHash::hash(state, (second, third, fourth, fifth))
    }
}

#[contract]
mod World {
    use array::ArrayTrait;
    use traits::Into;
    use hash::pedersen;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::contract_address_to_felt252;
    use starknet::ContractAddressZeroable;
    use super::IProxyDispatcher;
    use super::IProxyDispatcherTrait;
    use super::LegacyHashEntityRegsiteryTuple;

    struct Storage {
        num_entities: LegacyMap::<(felt252, felt252, felt252), felt252>,
        entity_registry_len: LegacyMap::<(starknet::ContractAddress, felt252, felt252, felt252), usize>,
        entity_registry: LegacyMap::<(starknet::ContractAddress, felt252, felt252, felt252, felt252), felt252>,
        module_registry: LegacyMap::<starknet::ContractAddress, bool>,
    }

    // Emitted anytime an entities component state is updated.
    #[event]
    fn ComponentValueSet(
        component_address: starknet::ContractAddress, entity_id: (felt252, felt252, felt252, felt252), data: Array::<felt252>
    ) {}

    // Emitted when a component or system is registered.
    #[event]
    fn ModuleRegistered(
        module_address: starknet::ContractAddress, module_id: felt252, class_hash: felt252
    ) {}

    // Register a component or system. The returned
    // hash is used to uniquely identify the component or
    // system in the world. All components and systems
    // within a world are deterministically addressed
    // relative to the world.
    #[external]
    fn register(class_hash: felt252, module_id: felt252) {
        let module_id = pedersen(0, module_id);
        let proxy_class_hash = starknet::class_hash_const::<0x420>();
        let calldata = ArrayTrait::<felt252>::new();
        let (module_address, _) = starknet::syscalls::deploy_syscall(
            proxy_class_hash, module_id, calldata.span(), bool::False(())
        ).unwrap_syscall();
        let world_address = get_contract_address();
        IProxyDispatcher { contract_address: module_address }.set_implementation(class_hash);
        IProxyDispatcher { contract_address: module_address }.initialize(world_address);
        module_registry::write(module_address, bool::True(()));
        ModuleRegistered(module_address, module_id, class_hash);
    }

    // Called when a component in the world updates the value
    // for an entity. When called for the first time for an 
    // entity, the entity:component mapping is registered.
    // Additionally, a `ComponentValueSet` event is emitted.
    #[external]
    fn on_component_set(entity_id: (felt252, felt252, felt252, felt252), data: Array::<felt252>) {
        let caller_address = get_caller_address();
        let (first, second, third, fourth) = entity_id;
        assert(module_registry::read(caller_address), 'component not a registered');
        let entities_len = entity_registry_len::read((caller_address, first, second, third));
        entity_registry::write((caller_address, first, second, third, entities_len.into()), fourth);
        entity_registry_len::write((caller_address, first, second, third), entities_len + 1_usize);
        ComponentValueSet(caller_address, entity_id, data);
    }

    // Issue an autoincremented id to the caller.
    #[external]
    fn next_entity_id(path: (felt252, felt252, felt252)) -> felt252 {
        let next = num_entities::read(path);
        num_entities::write(path, next + 1);
        return next;
    }

    // Returns entities that contain the component state.
    #[view]
    fn get_entities(component_address: starknet::ContractAddress, path: (felt252, felt252, felt252)) -> Array::<felt252> {
        let (first, second, third) = path;
        let entities_len = entity_registry_len::read((component_address, first, second, third));
        let mut entities = ArrayTrait::<felt252>::new();
        get_entities_inner(component_address, path, entities_len, ref entities);
        return entities;
    }

    fn get_entities_inner(
        component_address: starknet::ContractAddress,
        path: (felt252, felt252, felt252),
        entities_len: usize,
        ref entities: Array::<felt252>
    ) {
        match gas::withdraw_gas() {
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

        let (first, second, third) = path;
        let entity_id = entity_registry::read((component_address, first, second, third, (entities_len - 1_usize).into()));
        entities.append(entity_id);
        return get_entities_inner(component_address, path, entities_len - 1_usize, ref entities);
    }

}

#[test]
#[available_gas(2000000)]
fn test_on_component_set() { // World::register(420, 69);
// starknet_testing::set_caller_address(starknet::contract_address_const::<0x420>());
// let data = ArrayTrait::new();
// World::on_component_set(69_usize, data);
}

