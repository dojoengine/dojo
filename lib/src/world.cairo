use array::ArrayTrait;
use hash::LegacyHash;
use serde::Serde;

use starknet::contract_address::ContractAddressSerde;
use dojo::storage::StorageKey;
use dojo::storage::StorageKeyTrait;

#[abi]
trait IProxy {
    fn set_implementation(class_hash: felt252);
    fn initialize(world_address: starknet::ContractAddress);
}

#[abi]
trait IWorld {
    fn uuid() -> felt252;
    fn owner_of(entity_id: StorageKey) -> starknet::ContractAddress;
    fn read(component: starknet::ClassHash, key: StorageKey, offset: u8, length: u8) -> Span<felt252>;
    fn write(component: starknet::ClassHash, key: StorageKey, offset: u8, value: Span<felt252>);
    fn entities(component: starknet::ContractAddress, partition: felt252) -> Array<StorageKey>;
    fn has_role(role: felt252, account: starknet::ContractAddress) -> bool;
    fn grant_role(role: felt252, account: starknet::ContractAddress);
    fn revoke_role(role: felt252, account: starknet::ContractAddress);
    fn renounce_role(role: felt252, account: starknet::ContractAddress);
}

trait ComponentTrait<T> {
    fn initialize();
    fn set(entity_id: StorageKey, value: T);
    fn get(entity_id: StorageKey) -> T;
}

impl SpanSerde of Serde::<Span<felt252>> {
    fn serialize(ref serialized: Array<felt252>, mut input: Span<felt252>) {
        array::clone_loop(input, ref serialized);
    }
    fn deserialize(ref serialized: Span<felt252>) -> Option<Span<felt252>> {
        Option::Some(serialized)
    }
}

#[contract]
mod Executor {
    use super::SpanSerde;

    const EXECUTE_ENTRYPOINT: felt252 = 0x240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44;

    #[external]
    #[raw_output]
    fn execute(world: starknet::ContractAddress, class_hash: starknet::ClassHash, calldata: Span<felt252>) -> Span<felt252> {
        let res = starknet::syscalls::library_call_syscall(class_hash, EXECUTE_ENTRYPOINT, calldata).unwrap_syscall();
        res
    }
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
    use super::SpanSerde;

    use dojo::storage::StorageKey;
    use dojo::storage::LegacyHashClassHashStorageKey;
    use dojo::storage::StorageKeyIntoFelt252;

    use super::IProxyDispatcher;
    use super::IProxyDispatcherTrait;

    const EXECUTE_ENTRYPOINT: felt252 = 0x240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44;

    struct Storage {
        executor: starknet::ClassHash,
        partition_len: LegacyMap::<(felt252, felt252), usize>,
        partition: LegacyMap::<(felt252, felt252, felt252), felt252>,
        role_admin: LegacyMap::<felt252, felt252>,
        role_member: LegacyMap::<(felt252, starknet::ContractAddress), bool>,
        component_registry: LegacyMap::<felt252, ClassHash>,
        system_registry: LegacyMap::<felt252, ClassHash>,
        nonce: felt252,
    }

    // Emitted anytime an entities component state is updated.
    #[event]
    fn ComponentValueSet(
        component_address: starknet::ContractAddress, entity_id: StorageKey, data: Array::<felt252>
    ) {}

    #[event]
    fn ComponentRegistered(
        name: felt252, class_hash: ClassHash
    ) {}

    #[event]
    fn SystemRegistered(
        name: felt252, class_hash: ClassHash
    ) {}

    // Give deployer the default admin role.
    #[constructor]
    fn constructor() {
        let caller = get_caller_address();
        _grant_role(0, caller);
    }

    // Register a component or system. The returned
    // hash is used to uniquely identify the component or
    // system in the world. All components and systems
    // within a world are deterministically addressed
    // relative to the world.
    #[external]
    fn register_component(name: felt252, class_hash: ClassHash) {
        component_registry::write(name, class_hash);
        ComponentRegistered(name, class_hash);
    }

    #[external]
    fn register_system(name: felt252, class_hash: ClassHash) {
        system_registry::write(name, class_hash);
        SystemRegistered(name, class_hash);
    }

    #[external]
    fn execute(name: felt252, calldata: Span<felt252>) -> Span<felt252> {
        let class_hash = system_registry::read(name);
        executor::write(class_hash);
        let ret = starknet::syscalls::call_contract_syscall(starknet::contract_address_const::<0x420>(), EXECUTE_ENTRYPOINT, calldata).unwrap_syscall();
        executor::write(starknet::class_hash_const::<0x0>());
        ret
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
            hash::LegacyHash::<(felt252, StorageKey)>::hash(0x420, (component, key)))
    }

    #[view]
    fn read(component: felt252, key: StorageKey, offset: u8, length: u8) -> Span<felt252> {
        let address_domain = 0_u32;
        let base = address(component, key);
        let mut value = ArrayTrait::<felt252>::new();
        read_loop(address_domain, base, ref value, offset, length);
        value.span()
    }

    fn read_loop(address_domain: u32, base: starknet::StorageBaseAddress, ref value: Array<felt252>, offset: u8, length: u8) {
        match gas::withdraw_gas() {
            Option::Some(_) => {},
            Option::None(_) => {
                let mut data = ArrayTrait::new();
                data.append('Out of gas');
                panic(data);
            },
        }

        if offset == length {
            return ();
        }

        value.append(starknet::storage_read_syscall(
            address_domain, starknet::storage_address_from_base_and_offset(base, offset)
        ).unwrap_syscall());

        return read_loop(address_domain, base, ref value, offset + 1_u8, length);
    }

    #[external]
    fn write(component: felt252, key: StorageKey, offset: u8, value: Span<felt252>) {
        let executor_class_hash = executor::read();
        // TODO: verify executor has permission to write
        // TODO: bounds check
        let address_domain = 0_u32;
        let base = address(component, key);
        write_loop(address_domain, base, value, offset: offset);
    }

    fn write_loop(address_domain: u32, base: starknet::StorageBaseAddress, mut value: Span<felt252>, offset: u8) {
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
                write_loop(address_domain, base, value, offset + 1_u8);
            },
            Option::None(_) => {},
        }
    }

    // Returns entities that contain the component state.
    #[view]
    fn entities(component: felt252, partition: felt252) -> Array::<felt252> {
        let entities_len = partition_len::read((component, partition));
        let mut entities = ArrayTrait::<felt252>::new();
        entities_inner(component, partition, entities_len, ref entities);
        return entities;
    }

    fn entities_inner(
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
        return entities_inner(component, partition, entities_len - 1_usize, ref entities);
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

#[test]
#[available_gas(2000000)]
fn test_on_component_set() {
    World::register_component('Position', starknet::class_hash_const::<0x420>());
    // starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    // let data = ArrayTrait::new();
    // let id = World::uuinto();
    // let mut key = ArrayTrait::new();
    // key.append(id);
    // World::on_component_set(StorageKeyTrait::new(0, key), data);
}

#[test]
#[available_gas(2000000)]
fn test_constructor() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor();
    assert(World::has_role(0, starknet::contract_address_const::<0x420>()), 'role not granted');
}

#[test]
#[available_gas(2000000)]
fn test_grant_revoke_role() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor();
    World::grant_role(1, starknet::contract_address_const::<0x421>());
    assert(World::has_role(1, starknet::contract_address_const::<0x421>()), 'role not granted');
    World::revoke_role(1, starknet::contract_address_const::<0x421>());
    assert(!World::has_role(1, starknet::contract_address_const::<0x421>()), 'role not revoked');
}

#[test]
#[available_gas(2000000)]
fn test_renonce_role() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor();
    World::renounce_role(0);
    assert(!World::has_role(0, starknet::contract_address_const::<0x420>()), 'role not renonced');
}
