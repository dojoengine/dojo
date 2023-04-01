#[contract]
mod World {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::ClassHash;
    use starknet::ContractAddress;

    use dojo::serde::SpanSerde;
    use dojo::storage::key::StorageKey;
    use dojo::storage::key::StorageKeyTrait;
    use dojo::storage::key::StorageKeyIntoFelt252;

    use dojo::interfaces::IComponentLibraryDispatcher;
    use dojo::interfaces::IComponentDispatcherTrait;
    use dojo::interfaces::ISystemLibraryDispatcher;
    use dojo::interfaces::ISystemDispatcherTrait;
    use dojo::interfaces::IExecutorDispatcher;
    use dojo::interfaces::IExecutorDispatcherTrait;
    use dojo::interfaces::IIndexerLibraryDispatcher;
    use dojo::interfaces::IIndexerDispatcherTrait;
    use dojo::interfaces::IStoreLibraryDispatcher;
    use dojo::interfaces::IStoreDispatcherTrait;

    struct Storage {
        indexer: ClassHash,
        store: ClassHash,
        caller: ClassHash,
        executor: ContractAddress,
        component_registry: LegacyMap::<felt252, ClassHash>,
        system_registry: LegacyMap::<felt252, ClassHash>,
        nonce: felt252,
    }

    #[event]
    fn ComponentRegistered(name: felt252, class_hash: ClassHash) {}

    #[event]
    fn SystemRegistered(name: felt252, class_hash: ClassHash) {}

    // Give deployer the default admin role.
    #[constructor]
    fn constructor(executor_: ContractAddress, store_: ClassHash, indexer_: ClassHash) {
        executor::write(executor_);
        store::write(store_);
        indexer::write(indexer_);
    }

    // Register a component in the world. If the component is already registered,
    // the implementation will be updated.
    #[external]
    fn register_component(class_hash: ClassHash) {
        let name = IComponentLibraryDispatcher { class_hash: class_hash }.name();
        // TODO: If component is already registered, vaildate permission to update.
        component_registry::write(name, class_hash);
        ComponentRegistered(name, class_hash);
    }

    #[view]
    fn component(name: felt252) -> ClassHash {
        component_registry::read(name)
    }

    // Register a system in the world. If the system is already registered,
    // the implementation will be updated.
    #[external]
    fn register_system(class_hash: ClassHash) {
        let name = ISystemLibraryDispatcher { class_hash: class_hash }.name();
        // TODO: If system is already registered, vaildate permission to update.
        system_registry::write(name, class_hash);
        SystemRegistered(name, class_hash);
    }

    #[view]
    fn system(name: felt252) -> ClassHash {
        system_registry::read(name)
    }

    #[external]
    fn execute(name: felt252, execute_calldata: Span<felt252>) -> Span<felt252> {
        let class_hash = system_registry::read(name);
        caller::write(class_hash);

        let res = IExecutorDispatcher {
            contract_address: executor::read()
        }.execute(class_hash, execute_calldata);

        caller::write(starknet::class_hash_const::<0x0>());
        res
    }

    // Issue an autoincremented id to the caller.
    #[external]
    fn uuid() -> felt252 {
        let next = nonce::read();
        nonce::write(next + 1);
        return pedersen(next, 0);
    }

    #[external]
    fn set(component: felt252, key: StorageKey, offset: u8, value: Span<felt252>) {
        let system_class_hash = caller::read();
        let table = key.table(component);
        let id = key.id();

        // TODO: verify executor has permission to write

        let class_hash = component_registry::read(component);
        let res = IStoreLibraryDispatcher {
            class_hash: store::read()
        }.set(table, class_hash, key, offset, value);

        IIndexerLibraryDispatcher { class_hash: indexer::read() }.index(table, id);
    }

    #[view]
    fn get(component: felt252, key: StorageKey, offset: u8, mut length: usize) -> Span<felt252> {
        let class_hash = component_registry::read(component);

        let res = IStoreLibraryDispatcher {
            class_hash: store::read()
        }.get(component, class_hash, key, offset, length);

        res
    }

    // Returns entities that contain the component state.
    #[view]
    fn entities(component: felt252, partition: felt252) -> Array::<felt252> {
        if partition == 0 {
            return IIndexerLibraryDispatcher { class_hash: indexer::read() }.records(component);
        }

        IIndexerLibraryDispatcher {
            class_hash: indexer::read()
        }.records(pedersen(component, partition))
    }

    #[external]
    fn set_executor(contract_address: ContractAddress) {
        executor::write(contract_address);
    }

    #[external]
    fn set_indexer(class_hash: ClassHash) {
        indexer::write(class_hash);
    }

    #[external]
    fn set_store(class_hash: ClassHash) {
        store::write(class_hash);
    }
}

// TODO: Uncomment once library call is supported in tests.
// #[test]
// #[available_gas(2000000)]
// fn test_component() {
//     let name = 'Position';
//     World::register_component(starknet::class_hash_const::<0x420>());
//     let mut data = ArrayTrait::<felt252>::new();
//     data.append(1337);
//     let id = World::uuid();
//     World::set(name, StorageKeyTrait::new_from_id(id), 0_u8, data.span());
//     let stored = World::get(name, StorageKeyTrait::new_from_id(id), 0_u8, 1_usize);
//     assert(*stored.snapshot.at(0_usize) == 1337, 'data not stored');
// }

// TODO: Uncomment once library call is supported in tests.
// #[test]
// #[available_gas(2000000)]
// fn test_system() {
//     let name = 'Position';
//     World::register_system(starknet::class_hash_const::<0x420>());
//     let mut data = ArrayTrait::<felt252>::new();
//     data.append(1337);
//     let id = World::uuid();
//     World::set(name, StorageKeyTrait::new_from_id(id), 0_u8, data.span());
//     let stored = World::get(name, StorageKeyTrait::new_from_id(id), 0_u8, 1_usize);
//     assert(*stored.snapshot.at(0_usize) == 1337, 'data not stored');
// }

#[test]
#[available_gas(2000000)]
fn test_constructor() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor(
        starknet::contract_address_const::<0x1337>(),
        starknet::class_hash_const::<0x1337>(),
        starknet::class_hash_const::<0x1337>()
    );
}
