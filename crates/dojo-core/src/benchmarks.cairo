use core::result::ResultTrait;
use array::ArrayTrait;
use array::SpanTrait;
use debug::PrintTrait;
use option::OptionTrait;
use poseidon::poseidon_hash_span;
use starknet::SyscallResultTrait;
use starknet::{contract_address_const, ContractAddress, ClassHash, get_caller_address};

use dojo::database;
use dojo::database::{storage, index};
use dojo::model::Model;
use dojo::world_test::Foo;
use dojo::test_utils::end;


#[test]
#[available_gas(1000000000)]
fn bench_reference_offset() {
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    end(gas, 'bench empty');
}

#[test]
#[available_gas(1000000000)]
fn bench_storage_single() {
    let keys = array!['database_test', '42'].span();

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    storage::set(0, keys, 420);
    end(gas, 'storage set');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let res = storage::get(0, keys);
    end(gas, 'storage get');

    assert(res == 420, 'values differ');
}

#[test]
#[available_gas(1000000000)]
fn bench_storage_many() {
    let keys = array![0x1337].span();
    let values = array![1, 2].span();
    let layout = array![251, 251].span();

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    storage::set_many(0, keys, 0, values, layout);
    end(gas, 'storage set mny');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let res = storage::get_many(0, keys, 0, 2, layout);
    end(gas, 'storage get mny');

    assert(res.len() == 2, 'wrong number of values');
    assert(*res.at(0) == *values.at(0), 'value not set');
    assert(*res.at(1) == *values.at(1), 'value not set');
}

#[test]
#[available_gas(1000000000)]
fn bench_native_storage() {
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let keys = array![0x1337].span();
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    let address = starknet::storage_address_from_base(base);
    end(gas, 'native prep');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    starknet::storage_write_syscall(0, address, 42);
    end(gas, 'native write');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let value = starknet::storage_read_syscall(0, address).unwrap_syscall();
    end(gas, 'native read');

    assert(value == 42, 'read invalid');
}

#[test]
#[available_gas(1000000000)]
fn bench_native_storage_offset() {
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let keys = array![0x1337].span();
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    let address = starknet::storage_address_from_base_and_offset(base, 42);
    end(gas, 'native prep of');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    starknet::storage_write_syscall(0, address, 42);
    end(gas, 'native writ of');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let value = starknet::storage_read_syscall(0, address).unwrap_syscall();
    end(gas, 'native read of');

    assert(value == 42, 'read invalid');
}

#[test]
#[available_gas(1000000000)]
fn bench_index() {
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let no_query = index::query(0, 69, Option::None(()));
    end(gas, 'idx empty');
    assert(no_query.len() == 0, 'entity indexed');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    index::create(0, 69, 420);
    end(gas, 'idx create 1st');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let query = index::query(0, 69, Option::None(()));
    end(gas, 'idx query one');
    assert(query.len() == 1, 'entity not indexed');
    assert(*query.at(0) == 420, 'entity value incorrect');
    
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    index::create(0, 69, 1337);
    end(gas, 'idx query 2nd');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let two_query = index::query(0, 69, Option::None(()));
    end(gas, 'idx query two');
    assert(two_query.len() == 2, 'index should have two query');
    assert(*two_query.at(1) == 1337, 'entity value incorrect');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    index::exists(0, 69, 420);
    end(gas, 'idx exists chk');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    index::delete(0, 69, 420);
    end(gas, 'idx dlt !last');

    assert(!index::exists(0, 69, 420), 'entity should not exist');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    index::delete(0, 69, 1337);
    end(gas, 'idx dlt last');

    assert(!index::exists(0, 69, 1337), 'entity should not exist');
}

#[test]
#[available_gas(1000000000)]
fn bench_big_index() {
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let mut i = 0;
    loop {
        if i == 1000 {
            break;
        }
        index::create(0, 69, i);
        i += 1;
    };
    end(gas, 'idx create 1000');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let query = index::query(0, 69, Option::None(()));
    end(gas, 'idx query 1000');
    assert(query.len() == 1000, 'entity not indexed');
    assert(*query.at(420) == 420, 'entity value incorrect');
    
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    index::exists(0, 69, 999);
    end(gas, 'idx exists 1000');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    index::delete(0, 69, 999);
    end(gas, 'idx dlt 1000');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    index::delete(0, 69, 420);
    end(gas, 'idx dlt !1000 >');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    index::delete(0, 69, 420);
    end(gas, 'idx dlt !1000 0');
}

#[test]
#[available_gas(1000000000)]
fn bench_database_array() {
    let value = array![1, 2, 3, 4, 5, 6, 7, 8, 9, 10].span();
    let layout = array![251, 251, 251, 251, 251, 251, 251, 251, 251, 251].span();
    let half_layout = array![251, 251, 251, 251, 251].span();
    let len = value.len();

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    database::set('table', 'key', 0, value, layout);
    end(gas, 'db set arr');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let res = database::get('table', 'key', 0, len, layout);
    end(gas, 'db get arr');

    assert(res.len() == len, 'wrong number of values');
    assert(*res.at(0) == *value.at(0), 'value not set');
    assert(*res.at(1) == *value.at(1), 'value not set');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let second_res = database::get('table', 'key', 3, 8, array![251, 251, 251, 251, 251].span());
    end(gas, 'db get half arr');

    assert(second_res.len() == 5, 'wrong number of values');
    assert(*second_res.at(0) == *value.at(3), 'value not set');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    database::del('table', 'key');
    end(gas, 'db del arr');
}

#[test]
#[available_gas(1000000000)]
fn bench_indexed_database_array() {
    let even = array![2, 4].span();
    let odd = array![1, 3].span();
    let layout = array![251, 251].span();

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    database::set_with_index('table', 'even', 0, even, layout);
    end(gas, 'dbi set arr 1st');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let (keys, values) = database::scan('table', Option::None(()), 2, layout);
    end(gas, 'dbi scan arr 1');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    database::set_with_index('table', 'odd', 0, odd, layout);
    end(gas, 'dbi set arr 2nd');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let (keys, values) = database::scan('table', Option::None(()), 2, layout);
    end(gas, 'dbi scan arr 2');

    assert(keys.len() == 2, 'Wrong number of keys!');
    assert(values.len() == 2, 'Wrong number of values!');
    assert(*keys.at(0) == 'even', 'Wrong key at index 0!');
    assert(*(*values.at(0)).at(0) == 2, 'Wrong value at index 0!');
    assert(*(*values.at(0)).at(1) == 4, 'Wrong value at index 1!');
}


#[test]
#[available_gas(1000000000)]
fn bench_simple_struct() {
    let caller = starknet::contract_address_const::<0x42>();

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let mut foo = Foo {
        caller,
        a: 0x123456789abcdef,
        b: 0x123456789abcdef,
    };
    end(gas, 'foo init');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let mut serialized = ArrayTrait::new();
    serde::Serde::serialize(@foo.a, ref serialized);
    serde::Serde::serialize(@foo.b, ref serialized);
    let serialized = array::ArrayTrait::span(@serialized);
    end(gas, 'foo serialize');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let values: Span<felt252> = foo.values();
    end(gas, 'foo values');

    assert(serialized.len() == 2, 'serialized wrong length');
    assert(values.len() == 2, 'value wrong length');
    assert(serialized.at(0) == values.at(0), 'serialized differ at 0');
    assert(serialized.at(1) == values.at(1), 'serialized differ at 1');
}

#[derive(Model, Copy, Drop, Serde)]
struct PositionWithQuaterions {
    #[key]
    id: felt252,
    x: felt252,
    y: felt252,
    z: felt252,
    a: felt252,
    b: felt252,
    c: felt252,
    d: felt252,
}

#[test]
#[available_gas(1000000000)]
fn test_struct_with_many_fields() {
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();

    let mut pos = PositionWithQuaterions {
        id: 0x123456789abcdef,
        x: 0x123456789abcdef,
        y: 0x123456789abcdef,
        z: 0x123456789abcdef,
        a: 0x123456789abcdef,
        b: 0x123456789abcdef,
        c: 0x123456789abcdef,
        d: 0x123456789abcdef,
    };
    end(gas, 'pos init');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let mut serialized = ArrayTrait::new();
    serde::Serde::serialize(@pos.x, ref serialized);
    serde::Serde::serialize(@pos.y, ref serialized);
    serde::Serde::serialize(@pos.z, ref serialized);
    serde::Serde::serialize(@pos.a, ref serialized);
    serde::Serde::serialize(@pos.b, ref serialized);
    serde::Serde::serialize(@pos.c, ref serialized);
    serde::Serde::serialize(@pos.d, ref serialized);
    let serialized = array::ArrayTrait::span(@serialized);
    end(gas, 'pos serialize');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let values: Span<felt252> = pos.values();
    end(gas, 'pos values');

    assert(serialized.len() == values.len(), 'serialized not equal');
    let mut idx = 0;
    loop {
        if idx == serialized.len() {
            break;
        }
        assert(serialized.at(idx) == values.at(idx), 'serialized differ');
        idx += 1;
    };

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    database::set('positions', '42', 0, pos.values(), pos.layout());
    end(gas, 'pos db set');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    database::get('positions', '42', 0, pos.packed_size(), pos.layout());
    end(gas, 'pos db get');
}


#[derive(Introspect, Copy, Drop, Serde)]
struct Sword {
    swordsmith: ContractAddress,
    damage: u32,
}

#[derive(Model, Copy, Drop, Serde)]
struct Case {
    #[key]
    owner: ContractAddress,
    sword: Sword,
    material: felt252,
}


#[test]
#[available_gas(1000000000)]
fn bench_nested_struct() {
    let caller = starknet::contract_address_const::<0x42>();
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();

    let mut case = Case {
        owner: caller,
        sword: Sword {
            swordsmith: caller,
            damage: 0x12345678,
        },
        material: 'wooden',
    };
    end(gas, 'case init');
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();


    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let mut serialized = ArrayTrait::new();
    serde::Serde::serialize(@case.sword, ref serialized);
    serde::Serde::serialize(@case.material, ref serialized);
    let serialized = array::ArrayTrait::span(@serialized);
    end(gas, 'case serialize');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let values: Span<felt252> = case.values();
    end(gas, 'case values');

    assert(serialized.len() == values.len(), 'serialized not equal');
    let mut idx = 0;
    loop {
        if idx == serialized.len() {
            break;
        }
        assert(serialized.at(idx) == values.at(idx), 'serialized differ');
        idx += 1;
    };

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    database::set('cases', '42', 0, case.values(), case.layout());
    end(gas, 'case db set');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    database::get('cases', '42', 0, case.packed_size(), case.layout());
    end(gas, 'case db get');
}

#[derive(Model, Copy, Drop, Serde)]
struct Character {
    #[key]
    caller: ContractAddress,
    heigth: felt252,
    abilities: Abilities,
    stats: Stats,
    weapon: Weapon,
    gold: u32,
}

#[derive(Introspect, Copy, Drop, Serde)]
struct Abilities {
    strength: u8,
    dexterity: u8,
    constitution: u8,
    intelligence: u8,
    wisdom: u8,
    charisma: u8,
}

#[derive(Introspect, Copy, Drop, Serde)]
struct Stats {
    kills: u128,
    deaths: u16,
    rests: u32,
    hits: u64,
    blocks: u32,
    walked: felt252,
    runned: felt252,
    finished: bool,
    romances: u16,
}

#[derive(Introspect, Copy, Drop, Serde)]
enum Weapon {
    DualWield: (Sword, Sword),
    Fists: (Sword, Sword), // Introspect requires same arms
}

#[test]
#[available_gas(1000000000)]
fn bench_complex_struct() {
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();

    let char = Character {
        caller: starknet::contract_address_const::<0x42>(),
        heigth: 0x123456789abcdef,
        abilities: Abilities {
            strength: 0x12,
            dexterity: 0x34,
            constitution: 0x56,
            intelligence: 0x78,
            wisdom: 0x9a,
            charisma: 0xbc,
        },
        stats: Stats {
            kills: 0x123456789abcdef,
            deaths: 0x1234,
            rests: 0x12345678,
            hits: 0x123456789abcdef,
            blocks: 0x12345678,
            walked: 0x123456789abcdef,
            runned: 0x123456789abcdef,
            finished: true,
            romances: 0x1234,
        },
        weapon: Weapon::DualWield((
            Sword {
                swordsmith: starknet::contract_address_const::<0x69>(),
                damage: 0x12345678,
            },
            Sword {
                swordsmith: starknet::contract_address_const::<0x69>(),
                damage: 0x12345678,
            }
        )),
        gold: 0x12345678,
    };
    end(gas, 'chars init');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let mut serialized = ArrayTrait::new();
    serde::Serde::serialize(@char.heigth, ref serialized);
    serde::Serde::serialize(@char.abilities, ref serialized);
    serde::Serde::serialize(@char.stats, ref serialized);
    serde::Serde::serialize(@char.weapon, ref serialized);
    serde::Serde::serialize(@char.gold, ref serialized);
    let serialized = array::ArrayTrait::span(@serialized);
    end(gas, 'chars serialize');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let values: Span<felt252> = char.values();
    end(gas, 'chars values');

    assert(serialized.len() == values.len(), 'serialized not equal');

    let mut idx = 0;
    loop {
        if idx == serialized.len() {
            break;
        }
        assert(serialized.at(idx) == values.at(idx), 'serialized differ');
        idx += 1;
    };

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    database::set('chars', '42', 0, char.values(), char.layout());
    end(gas, 'chars db set');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    database::get('chars', '42', 0, char.packed_size(), char.layout());
    end(gas, 'chars db get');
}