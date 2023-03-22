use array::ArrayTrait;
use hash::LegacyHash;
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

    use dojo::storage::StorageKey;
    use dojo::storage::LegacyHashStorageKey;

    use super::IProxyDispatcher;
    use super::IProxyDispatcherTrait;

    struct Storage {
        nonce: felt252,
        partition_len: LegacyMap::<(felt252, felt252), usize>,
        partition: LegacyMap::<(felt252, felt252, felt252), felt252>,
        module_registry: LegacyMap::<starknet::ContractAddress, bool>,
        role_admin: LegacyMap::<felt252, felt252>,
        role_member: LegacyMap::<(felt252, starknet::ContractAddress), bool>,
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
    World::register(420, 69);
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    let data = ArrayTrait::new();
    let id = World::uuid();
    let mut key = ArrayTrait::new();
    key.append(id);
    World::on_component_set(StorageKeyTrait::new(0, key), data);
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
