#[contract]
mod World {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::class_hash::ClassHash;

    use dojo::serde::SpanSerde;
    use dojo::storage::key::StorageKey;
    use dojo::storage::key::StorageKeyTrait;
    use dojo::storage::key::StorageKeyIntoFelt252;

    use dojo::interfaces::IComponentLibraryDispatcher;
    use dojo::interfaces::IComponentDispatcherTrait;
    use dojo::interfaces::IExecutorDispatcher;
    use dojo::interfaces::IExecutorDispatcherTrait;
    use dojo::interfaces::IIndexerLibraryDispatcher;
    use dojo::interfaces::IIndexerDispatcherTrait;
    use dojo::interfaces::IStoreLibraryDispatcher;
    use dojo::interfaces::IStoreDispatcherTrait;

    struct Storage {
        indexer: starknet::ClassHash,
        store: starknet::ClassHash,
        caller: starknet::ClassHash,
        executor: starknet::ContractAddress,
        role_admin: LegacyMap::<felt252, felt252>,
        role_member: LegacyMap::<(felt252, starknet::ContractAddress), bool>,
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
    fn constructor(_executor: starknet::ContractAddress) {
        executor::write(_executor);
        let caller = get_caller_address();
        _grant_role(0, caller);
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

    // Register a system in the world. If the system is already registered,
    // the implementation will be updated.
    #[external]
    fn register_system(class_hash: ClassHash) {
        let name = IComponentLibraryDispatcher { class_hash: class_hash }.name();
        // TODO: If system is already registered, vaildate permission to update.
        system_registry::write(name, class_hash);
        SystemRegistered(name, class_hash);
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

    #[view]
    fn get(component: felt252, key: StorageKey, offset: u8, mut length: usize) -> Span<felt252> {
        let class_hash = component_registry::read(component);

        let res = IStoreLibraryDispatcher {
            class_hash: store::read()
        }.get(component, class_hash, key, offset, length);

        res
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

    // Returns entities that contain the component state.
    #[view]
    fn entities(component: felt252, partition: felt252) -> Array::<felt252> {
        if partition == 0 {
            return IIndexerLibraryDispatcher { class_hash: indexer::read() }.records(component);
        }

        IIndexerLibraryDispatcher { class_hash: indexer::read() }.records(pedersen(component, partition))
    }

    #[view]
    fn has_role(role: felt252, account: starknet::ContractAddress) -> bool {
        return role_member::read((role, account));
    }

    #[external]
    fn grant_role(role: felt252, account: starknet::ContractAddress) {
        let admin = role_admin::read(role);
        assert_only_role(admin);
        _grant_role(role, account);
    }

    fn _grant_role(role: felt252, account: starknet::ContractAddress) {
        let has_role = role_member::read((role, account));
        if (!has_role) {
            role_member::write((role, account), bool::True(()));
        }
    }

    #[external]
    fn revoke_role(role: felt252, account: starknet::ContractAddress) {
        let admin = role_admin::read(role);
        assert_only_role(admin);
        _revoke_role(role, account);
    }

    fn _revoke_role(role: felt252, account: starknet::ContractAddress) {
        let has_role = role_member::read((role, account));
        if (has_role) {
            role_member::write((role, account), bool::False(()));
        }
    }

    #[external]
    fn renounce_role(role: felt252) {
        let caller_address = get_caller_address();
        _revoke_role(role, caller_address);
    }

    fn assert_only_role(role: felt252) {
        let caller_address = get_caller_address();
        let has_role = has_role(role, caller_address);
        assert(has_role, 'caller is missing role');
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
    World::constructor(starknet::contract_address_const::<0x1337>());
    assert(World::has_role(0, starknet::contract_address_const::<0x420>()), 'role not granted');
}

#[test]
#[available_gas(2000000)]
fn test_grant_revoke_role() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor(starknet::contract_address_const::<0x1337>());
    World::grant_role(1, starknet::contract_address_const::<0x421>());
    assert(World::has_role(1, starknet::contract_address_const::<0x421>()), 'role not granted');
    World::revoke_role(1, starknet::contract_address_const::<0x421>());
    assert(!World::has_role(1, starknet::contract_address_const::<0x421>()), 'role not revoked');
}

#[test]
#[available_gas(2000000)]
fn test_renonce_role() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor(starknet::contract_address_const::<0x1337>());
    World::renounce_role(0);
    assert(!World::has_role(0, starknet::contract_address_const::<0x420>()), 'role not renonced');
}
