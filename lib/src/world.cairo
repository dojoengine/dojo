use array::ArrayTrait;
use hash::LegacyHash;
use starknet::contract_address::ContractAddressSerde;
use dojo::storage::StorageKey;
use dojo::storage::StorageKeyTrait;
use dojo::storage::PartitionKeyTrait;

#[abi]
trait IProxy {
    fn set_implementation(class_hash: felt252);
    fn initialize(world_address: starknet::ContractAddress);
}

#[abi]
trait IWorld {
    fn uuid() -> felt252;
    fn owner_of(entity_id: StorageKey) -> starknet::ContractAddress;
    fn entities(
        component: starknet::ContractAddress, entity_id: (felt252, felt252, felt252)
    ) -> Array<StorageKey>;
}

trait ComponentTrait<T> {
    fn initialize();
    fn set(entity_id: StorageKey, value: T);
    fn get(entity_id: StorageKey) -> T;
}

impl LegacyHashEntityRegsiteryTuple of LegacyHash::<(
    starknet::ContractAddress, felt252, felt252, felt252, felt252
)> {
    fn hash(
        state: felt252, tuple: (starknet::ContractAddress, felt252, felt252, felt252, felt252)
    ) -> felt252 {
        let (first, second, third, fourth, fifth) = tuple;
        let state = LegacyHash::hash(state, first);
        LegacyHash::hash(state, (second, third, fourth, fifth))
    }
}

#[contract]
mod World {
    use array::ArrayTrait;
    use box::BoxTrait;
    use traits::Into;
    use hash::pedersen;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::contract_address_to_felt252;
    use starknet::ContractAddressZeroable;
    use starknet::ContractAddressIntoFelt252;

    use dojo::storage::PartitionKey;
    use dojo::storage::StorageKey;
    use dojo::storage::LegacyHashStorageKey;

    use super::IProxyDispatcher;
    use super::IProxyDispatcherTrait;

    struct Storage {
        nonce: felt252,
        partition_len: LegacyMap::<PartitionKey, usize>,
        partition: LegacyMap::<(PartitionKey, felt252), felt252>,
        module_registry: LegacyMap::<starknet::ContractAddress, bool>,
    }

    // Emitted anytime an entities component state is updated.
    #[event]
    fn ComponentValueSet(
        component_address: starknet::ContractAddress, entity_id: StorageKey, data: Array::<felt252>
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
        // let (module_address, _) = starknet::syscalls::deploy_syscall(
        //     proxy_class_hash, module_id, calldata.span(), bool::False(())
        // ).unwrap_syscall();

        let module_address = starknet::contract_address_const::<0x420>();
        let world_address = get_contract_address();
        // IProxyDispatcher { contract_address: module_address }.set_implementation(class_hash);
        // IProxyDispatcher { contract_address: module_address }.initialize(world_address);
        module_registry::write(module_address, bool::True(()));
        ModuleRegistered(module_address, module_id, class_hash);
    }

    // Called when a component in the world updates the value
    // for an entity. When called for the first time for an 
    // entity, the entity:component mapping is registered.
    // Additionally, a `ComponentValueSet` event is emitted.
    #[external]
    fn on_component_set(entity_id: StorageKey, data: Array::<felt252>) {
        let caller_address = get_caller_address();
        assert(module_registry::read(caller_address), 'component not a registered');
        // let entities_len = partition_len::read((caller_address, first, second, third));
        // partition::write((caller_address, first, second, third, entities_len.into()), fourth);
        // partition_len::write((caller_address, first, second, third), entities_len + 1_usize);
        ComponentValueSet(caller_address, entity_id, data);
    }

    // Issue an autoincremented id to the caller.
    #[external]
    fn uuid() -> felt252 {
        let next = nonce::read();
        nonce::write(next + 1);
        return pedersen(next, 0);
    }

    // Returns entities that contain the component state.
    #[view]
    fn entities(partition: PartitionKey, ) -> Array::<felt252> {
        let entities_len = partition_len::read(partition);
        let mut entities = ArrayTrait::<felt252>::new();
        entities_inner(partition, entities_len, ref entities);
        return entities;
    }

    fn entities_inner(
        partition: PartitionKey, entities_len: usize, ref entities: Array::<felt252>
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

        let entity_id = partition::read((partition, entities_len.into()));
        entities.append(entity_id);
        return entities_inner(partition, entities_len - 1_usize, ref entities);
    }
}

#[test]
#[available_gas(2000000)]
fn test_on_component_set() {
    World::register(420, 69);
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    let data = ArrayTrait::new();
    let id = World::uuid();
    let mut key = ArrayTrait::new();
    key.append(id);
    World::on_component_set(StorageKeyTrait::new(PartitionKeyTrait::new(0, 0), key), data);
}
