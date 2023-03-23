use array::ArrayTrait;
use hash::LegacyHash;
use serde::Serde;

use starknet::contract_address::ContractAddressSerde;
use dojo::storage::StorageKey;
use dojo::storage::StorageKeyTrait;
use dojo::serde::SpanSerde;

#[abi]
trait IWorld {
    fn register_component(class_hash: starknet::ClassHash);
    fn register_system(class_hash: starknet::ClassHash);
    fn uuid() -> felt252;
    fn get(
        component: felt252, key: dojo::storage::StorageKey, offset: u8, length: usize
    ) -> Span<felt252>;
    fn set(component: felt252, key: dojo::storage::StorageKey, offset: u8, value: Span<felt252>);
    fn all(component: felt252, partition: felt252) -> Array<dojo::storage::StorageKey>;
    fn has_role(role: felt252, account: starknet::ContractAddress) -> bool;
    fn grant_role(role: felt252, account: starknet::ContractAddress);
    fn revoke_role(role: felt252, account: starknet::ContractAddress);
    fn renounce_role(role: felt252, account: starknet::ContractAddress);
}

#[abi]
trait IComponent {
    fn name() -> felt252;
    fn len() -> usize;
}

#[abi]
trait ISystem {
    fn name() -> felt252;
}

#[contract]
mod World {
    use array::ArrayTrait;
    use array::SpanTrait;
    use box::BoxTrait;
    use traits::Into;
    use hash::pedersen;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::contract_address_to_felt252;
    use starknet::ContractAddressZeroable;
    use starknet::ContractAddressIntoFelt252;
    use starknet::class_hash::ClassHash;

    use dojo::executor::IExecutorDispatcher;
    use dojo::executor::IExecutorDispatcherTrait;
    use dojo::serde::SpanSerde;
    use dojo::storage::StorageKey;
    use dojo::storage::LegacyHashClassHashStorageKey;
    use dojo::storage::StorageKeyIntoFelt252;

    use super::IComponentLibraryDispatcher;
    use super::IComponentDispatcherTrait;

    struct Storage {
        caller: starknet::ClassHash,
        executor: starknet::ContractAddress,
        partition_len: LegacyMap::<(felt252, felt252), usize>,
        partition: LegacyMap::<(felt252, felt252, felt252), felt252>,
        role_admin: LegacyMap::<felt252, felt252>,
        role_member: LegacyMap::<(felt252, starknet::ContractAddress), bool>,
        component_registry: LegacyMap::<felt252, ClassHash>,
        system_registry: LegacyMap::<felt252, ClassHash>,
        nonce: felt252,
    }

    #[event]
    fn ValueSet(component: felt252, key: StorageKey, offset: u8, value: Span<felt252>) {}

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
    fn execute(name: felt252, calldata: Span<felt252>) -> Span<felt252> {
        let class_hash = system_registry::read(name);
        caller::write(class_hash);
        let res = IExecutorDispatcher {
            contract_address: executor::read()
        }.execute(starknet::get_contract_address(), class_hash, calldata);
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

    fn address(component: felt252, key: StorageKey) -> starknet::StorageBaseAddress {
        starknet::storage_base_address_from_felt252(
            hash::LegacyHash::<(felt252, StorageKey)>::hash(0x420, (component, key))
        )
    }

    #[view]
    fn get(component: felt252, key: StorageKey, offset: u8, mut length: usize) -> Span<felt252> {
        let address_domain = 0_u32;
        let base = address(component, key);
        let mut value = ArrayTrait::<felt252>::new();

        if length == 0_usize {
            length = IComponentLibraryDispatcher {
                class_hash: component_registry::read(component)
            }.len()
        }

        get_loop(address_domain, base, ref value, offset, length);
        value.span()
    }

    fn get_loop(
        address_domain: u32,
        base: starknet::StorageBaseAddress,
        ref value: Array<felt252>,
        offset: u8,
        length: usize
    ) {
        match gas::withdraw_gas() {
            Option::Some(_) => {},
            Option::None(_) => {
                let mut data = ArrayTrait::new();
                data.append('Out of gas');
                panic(data);
            },
        }

        if length.into() == offset.into() {
            return ();
        }

        value.append(
            starknet::storage_read_syscall(
                address_domain, starknet::storage_address_from_base_and_offset(base, offset)
            ).unwrap_syscall()
        );

        return get_loop(address_domain, base, ref value, offset + 1_u8, length);
    }

    #[external]
    fn set(component: felt252, key: StorageKey, offset: u8, value: Span<felt252>) {
        let _caller = caller::read();

        // TODO: verify executor has permission to write
        // TODO: Enable bounds check once we can use library calls in tests.
        // let length = IComponentLibraryDispatcher { class_hash: component_registry::read(component) }.len();
        // assert(value.len() <= length, 'Value too long');
        let address_domain = 0_u32;
        let base = address(component, key);
        set_loop(address_domain, base, value, offset: offset);
    // ValueSet(component, key, offset, value);
    }

    fn set_loop(
        address_domain: u32,
        base: starknet::StorageBaseAddress,
        mut value: Span<felt252>,
        offset: u8
    ) {
        match gas::withdraw_gas() {
            Option::Some(_) => {},
            Option::None(_) => {
                let mut data = ArrayTrait::new();
                data.append('Out of gas');
                panic(data);
            },
        }
        match value.pop_front() {
            Option::Some(v) => {
                starknet::storage_write_syscall(
                    address_domain, starknet::storage_address_from_base_and_offset(base, offset), *v
                );
                set_loop(address_domain, base, value, offset + 1_u8);
            },
            Option::None(_) => {},
        }
    }

    // Returns entities that contain the component state.
    #[view]
    fn all(component: felt252, partition: felt252) -> Array::<felt252> {
        let entities_len = partition_len::read((component, partition));
        let mut entities = ArrayTrait::<felt252>::new();
        all_inner(component, partition, entities_len, ref entities);
        return entities;
    }

    fn all_inner(
        component: felt252, partition: felt252, entities_len: usize, ref entities: Array::<felt252>
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

        let entity_id = partition::read((component, partition, entities_len.into()));
        entities.append(entity_id);
        return all_inner(component, partition, entities_len - 1_usize, ref entities);
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
