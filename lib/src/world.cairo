use array::ArrayTrait;
use hash::LegacyHash;
use serde::Serde;

use starknet::contract_address::ContractAddressSerde;
use dojo::storage::StorageKey;
use dojo::storage::StorageKeyTrait;
use dojo::module::ModuleIDTrait;

#[abi]
trait IProxy {
    fn set_implementation(class_hash: felt252);
    fn initialize(world_address: starknet::ContractAddress);
}

#[abi]
trait IWorld {
    fn uuid() -> felt252;
    fn owner_of(entity_id: StorageKey) -> starknet::ContractAddress;
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
    use starknet::class_hash::ClassHash;
    use super::SpanSerde;

    use dojo::storage::StorageKey;
    use dojo::storage::LegacyHashStorageKey;
    use dojo::module::ModuleID;
    use dojo::module::ModuleIDTrait;
    use dojo::module::LegacyHashModuleID;

    use super::IProxyDispatcher;
    use super::IProxyDispatcherTrait;

    struct Storage {
        nonce: felt252,
        partition_len: LegacyMap::<(felt252, felt252), usize>,
        partition: LegacyMap::<(felt252, felt252, felt252), felt252>,
        role_admin: LegacyMap::<felt252, felt252>,
        role_member: LegacyMap::<(felt252, starknet::ContractAddress), bool>,
        module_registry: LegacyMap::<ModuleID, ClassHash>,
    }

    // Emitted anytime an entities component state is updated.
    #[event]
    fn ComponentValueSet(
        component_address: starknet::ContractAddress, entity_id: StorageKey, data: Array::<felt252>
    ) {}

    // Emitted when a component or system is registered.
    #[event]
    fn ModuleRegistered(
        module_id: ModuleID, class_hash: ClassHash
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
        let module_id = ModuleIDTrait::new(0_u8, name);
        module_registry::write(module_id, class_hash);
        ModuleRegistered(module_id, class_hash);
    }

    #[external]
    fn register_system(name: felt252, class_hash: ClassHash) {
        let module_id = ModuleIDTrait::new(1_u8, name);
        module_registry::write(module_id, class_hash);
        ModuleRegistered(module_id, class_hash);
    }

    #[external]
    fn execute(name: felt252, calldata: Span<felt252>) -> Span<felt252> {
        let class_hash = module_registry::read(ModuleIDTrait::new(1_u8, name));
        starknet::syscalls::library_call_syscall(class_hash, 0x420, calldata).unwrap_syscall()
    }

    // Called when a component in the world updates the value
    // for an entity. When called for the first time for an 
    // entity, the entity:component mapping is registered.
    // Additionally, a `ComponentValueSet` event is emitted.
    #[external]
    fn on_component_set(entity_id: StorageKey, data: Array::<felt252>) {
        let caller_address = get_caller_address();
        // assert(module_registry::read(caller_address), 'component not a registered');
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
    World::register_component('position', starknet::class_hash_const::<0x420>());
    // starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    // let data = ArrayTrait::new();
    // let id = World::uuid();
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
