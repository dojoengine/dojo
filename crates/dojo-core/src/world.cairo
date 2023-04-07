#[contract]
mod World {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::ClassHash;
    use starknet::ContractAddress;

    use dojo_core::serde::SpanSerde;
    use dojo_core::storage::key::StorageKey;
    use dojo_core::storage::key::StorageKeyTrait;
    use dojo_core::storage::key::StorageKeyIntoFelt252;
    use dojo_core::storage::db::Database;

    use dojo_core::interfaces::IComponentLibraryDispatcher;
    use dojo_core::interfaces::IComponentDispatcherTrait;
    use dojo_core::interfaces::IExecutorDispatcher;
    use dojo_core::interfaces::IExecutorDispatcherTrait;
    use dojo_core::interfaces::IIndexerLibraryDispatcher;
    use dojo_core::interfaces::IIndexerDispatcherTrait;
    use dojo_core::interfaces::IWorld;

    struct Storage {
        caller: ClassHash,
        executor: ContractAddress,
        role_admin: LegacyMap::<felt252, felt252>,
        role_member: LegacyMap::<(felt252, ContractAddress), bool>,
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
    fn constructor(executor_: ContractAddress, indexer_: ClassHash) {
        let caller = get_caller_address();
        _grant_role(0, caller);

        executor::write(executor_);
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
        let name = IComponentLibraryDispatcher { class_hash: class_hash }.name();
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
    fn set_entity(component: felt252, key: StorageKey, offset: u8, value: Span<felt252>) {
        let system_class_hash = caller::read();
        let table = key.table(component);

        // TODO: verify executor has permission to write

        let class_hash = component_registry::read(component);
        let length = IComponentLibraryDispatcher { class_hash: class_hash }.len();
        assert(value.len() <= length, 'Value too long');

        Database::set(class_hash, table, key, offset, value)
    }

    #[external]
    fn delete_entity(component: felt252, key: StorageKey) {
        let class_hash = caller::read();
        let res = Database::del(class_hash, component, key);
    }

    #[view]
    fn entity(component: felt252, key: StorageKey, offset: u8, mut length: usize) -> Span<felt252> {
        let class_hash = component_registry::read(component);

        if length == 0_usize {
            length = IComponentLibraryDispatcher { class_hash: class_hash }.len()
        }

        let res = Database::get(class_hash, component, key, offset, length);

        res
    }

    // Returns entities that contain the component state.
    #[view]
    fn entities(component: felt252, partition: felt252) -> Array::<felt252> {
        Database::all(component, partition)
    }

    #[external]
    fn set_executor(contract_address: ContractAddress) {
        executor::write(contract_address);
    }

    #[view]
    fn has_role(role: felt252, account: ContractAddress) -> bool {
        return role_member::read((role, account));
    }

    #[external]
    fn grant_role(role: felt252, account: ContractAddress) {
        let admin = role_admin::read(role);
        assert_only_role(admin);
        _grant_role(role, account);
    }

    #[external]
    fn revoke_role(role: felt252, account: ContractAddress) {
        let admin = role_admin::read(role);
        assert_only_role(admin);
        _revoke_role(role, account);
    }

    #[external]
    fn renounce_role(role: felt252) {
        let caller_address = get_caller_address();
        _revoke_role(role, caller_address);
    }

    fn _grant_role(role: felt252, account: ContractAddress) {
        let has_role = role_member::read((role, account));
        if (!has_role) {
            role_member::write((role, account), bool::True(()));
        }
    }

    fn _revoke_role(role: felt252, account: ContractAddress) {
        let has_role = role_member::read((role, account));
        if (has_role) {
            role_member::write((role, account), bool::False(()));
        }
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
//     World::set_entity(name, StorageKeyTrait::new_from_id(id), 0_u8, data.span());
//     let stored = World::entity(name, StorageKeyTrait::new_from_id(id), 0_u8, 1_usize);
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
//     World::set_entity(name, StorageKeyTrait::new_from_id(id), 0_u8, data.span());
//     let stored = World::entity(name, StorageKeyTrait::new_from_id(id), 0_u8, 1_usize);
//     assert(*stored.snapshot.at(0_usize) == 1337, 'data not stored');
// }

#[test]
#[available_gas(2000000)]
fn test_constructor() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor(starknet::contract_address_const::<0x1337>(), starknet::class_hash_const::<0x1337>());
    assert(World::has_role(0, starknet::contract_address_const::<0x420>()), 'role not granted');
}

#[test]
#[available_gas(2000000)]
fn test_grant_revoke_role() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor(starknet::contract_address_const::<0x1337>(), starknet::class_hash_const::<0x1337>());
    World::grant_role(1, starknet::contract_address_const::<0x421>());
    assert(World::has_role(1, starknet::contract_address_const::<0x421>()), 'role not granted');
    World::revoke_role(1, starknet::contract_address_const::<0x421>());
    assert(!World::has_role(1, starknet::contract_address_const::<0x421>()), 'role not revoked');
}

#[test]
#[available_gas(2000000)]
fn test_renonce_role() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor(starknet::contract_address_const::<0x1337>(), starknet::class_hash_const::<0x1337>());
    World::renounce_role(0);
    assert(!World::has_role(0, starknet::contract_address_const::<0x420>()), 'role not renonced');
}
