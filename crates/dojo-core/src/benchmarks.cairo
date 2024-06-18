use core::result::ResultTrait;
use array::ArrayTrait;
use array::SpanTrait;
use debug::PrintTrait;
use poseidon::poseidon_hash_span;
use starknet::SyscallResultTrait;
use starknet::{contract_address_const, ContractAddress, ClassHash, get_caller_address};

use dojo::database;
use dojo::database::storage;
use dojo::model::Model;
use dojo::world_test::Foo;
use dojo::test_utils::GasCounterImpl;
use dojo::database::introspect::{Introspect, Layout};

#[test]
#[available_gas(1000000000)]
fn bench_reference_offset() {
    let gas = GasCounterImpl::start();
    gas.end("bench empty");
}

#[test]
#[available_gas(1000000000)]
fn bench_storage_single() {
    let keys = array!['database_test', '42'].span();

    let gas = GasCounterImpl::start();
    storage::set(0, keys, 420);
    gas.end("storage set");

    let gas = GasCounterImpl::start();
    let res = storage::get(0, keys);
    gas.end("storage get");

    assert(res == 420, 'values differ');
}

#[test]
#[available_gas(1000000000)]
fn bench_storage_many() {
    let keys = array![0x1337].span();
    let values = array![1, 2].span();
    let layout = array![251, 251].span();

    let gas = GasCounterImpl::start();
    storage::set_many(0, keys, values, 0, layout).unwrap();
    gas.end("storage set_many");

    let gas = GasCounterImpl::start();
    let res = storage::get_many(0, keys, layout).unwrap();
    gas.end("storage get_many");

    assert(res.len() == 2, 'wrong number of values');
    assert(*res.at(0) == *values.at(0), 'value not set');
    assert(*res.at(1) == *values.at(1), 'value not set');
}

#[test]
#[available_gas(1000000000)]
fn bench_native_storage() {
    let gas = GasCounterImpl::start();
    let keys = array![0x1337].span();
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    let address = starknet::storage_address_from_base(base);
    gas.end("native prep");

    let gas = GasCounterImpl::start();
    starknet::storage_write_syscall(0, address, 42).unwrap_syscall();
    gas.end("native write");

    let gas = GasCounterImpl::start();
    let value = starknet::storage_read_syscall(0, address).unwrap_syscall();
    gas.end("native read");

    assert(value == 42, 'read invalid');
}

#[test]
#[available_gas(1000000000)]
fn bench_native_storage_offset() {
    let gas = GasCounterImpl::start();
    let keys = array![0x1337].span();
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    let address = starknet::storage_address_from_base_and_offset(base, 42);
    gas.end("native prep of");

    let gas = GasCounterImpl::start();
    starknet::storage_write_syscall(0, address, 42).unwrap_syscall();
    gas.end("native writ of");

    let gas = GasCounterImpl::start();
    let value = starknet::storage_read_syscall(0, address).unwrap_syscall();
    gas.end("native read of");

    assert(value == 42, 'read invalid');
}

#[test]
#[available_gas(1000000000)]
fn bench_database_array() {
    let mut keys = ArrayTrait::new();
    keys.append(0x966);

    let array_test_len: usize = 300;

    let mut layout = ArrayTrait::new();
    let mut values: Array<felt252> = ArrayTrait::new();
    let mut i = 0;
    loop {
        if i == array_test_len {
            break;
        }

        values.append(i.into());
        layout.append(251_u8);

        i += 1;
    };

    let gas = GasCounterImpl::start();
    database::set('table', 'key', values.span(), 0, layout.span());
    gas.end("db set arr");

    let gas = GasCounterImpl::start();
    let res = database::get('table', 'key', layout.span());
    gas.end("db get arr");

    let mut i = 0;
    loop {
        if i == array_test_len {
            break;
        }

        assert(res.at(i) == values.at(i), 'Value not equal!');
        i += 1;
    };
}

#[test]
#[available_gas(1000000000)]
fn bench_simple_struct() {
    let caller = starknet::contract_address_const::<0x42>();

    let gas = GasCounterImpl::start();
    let mut foo = Foo { caller, a: 0x123456789abcdef, b: 0x123456789abcdef, };
    gas.end("foo init");

    let gas = GasCounterImpl::start();
    let mut serialized = ArrayTrait::new();
    serde::Serde::serialize(@foo.a, ref serialized);
    serde::Serde::serialize(@foo.b, ref serialized);
    let serialized = array::ArrayTrait::span(@serialized);
    gas.end("foo serialize");

    let gas = GasCounterImpl::start();
    let values: Span<felt252> = foo.values();
    gas.end("foo values");

    assert(serialized.len() == 2, 'serialized wrong length');
    assert(values.len() == 2, 'value wrong length');
    assert(serialized.at(0) == values.at(0), 'serialized differ at 0');
    assert(serialized.at(1) == values.at(1), 'serialized differ at 1');
}

#[derive(Copy, Drop, Serde, IntrospectPacked)]
#[dojo::model]
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

// TODO: this test should be adapted to benchmark the new layout system
#[test]
#[available_gas(1000000000)]
fn test_struct_with_many_fields_fixed() {
    let gas = GasCounterImpl::start();

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
    gas.end("pos init");

    let gas = GasCounterImpl::start();
    let mut serialized = ArrayTrait::new();
    serde::Serde::serialize(@pos.x, ref serialized);
    serde::Serde::serialize(@pos.y, ref serialized);
    serde::Serde::serialize(@pos.z, ref serialized);
    serde::Serde::serialize(@pos.a, ref serialized);
    serde::Serde::serialize(@pos.b, ref serialized);
    serde::Serde::serialize(@pos.c, ref serialized);
    serde::Serde::serialize(@pos.d, ref serialized);
    let serialized = array::ArrayTrait::span(@serialized);
    gas.end("pos serialize");

    let gas = GasCounterImpl::start();
    let values: Span<felt252> = pos.values();
    gas.end("pos values");

    assert(serialized.len() == values.len(), 'serialized not equal');
    let mut idx = 0;
    loop {
        if idx == serialized.len() {
            break;
        }
        assert(serialized.at(idx) == values.at(idx), 'serialized differ');
        idx += 1;
    };

    let layout = match dojo::model::Model::<PositionWithQuaterions>::layout() {
        Layout::Fixed(layout) => layout,
        _ => panic!("expected fixed layout"),
    };

    let gas = GasCounterImpl::start();
    database::set('positions', '42', pos.values(), 0, layout);
    gas.end("pos db set");

    let gas = GasCounterImpl::start();
    database::get('positions', '42', layout);
    gas.end("pos db get");
}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
struct Sword {
    swordsmith: ContractAddress,
    damage: u32,
}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
#[dojo::model]
struct Case {
    #[key]
    owner: ContractAddress,
    sword: Sword,
    material: felt252,
}

// TODO: this test should be adapted to benchmark the new layout system
#[test]
#[ignore]
#[available_gas(1000000000)]
fn bench_nested_struct_packed() {
    let caller = starknet::contract_address_const::<0x42>();

    let gas = GasCounterImpl::start();
    let mut case = Case {
        owner: caller, sword: Sword { swordsmith: caller, damage: 0x12345678, }, material: 'wooden',
    };
    gas.end("case init");

    // ????
    let _gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();

    let gas = GasCounterImpl::start();
    let mut serialized = ArrayTrait::new();
    serde::Serde::serialize(@case.sword, ref serialized);
    serde::Serde::serialize(@case.material, ref serialized);
    let serialized = array::ArrayTrait::span(@serialized);
    gas.end("case serialize");

    let gas = GasCounterImpl::start();
    let values: Span<felt252> = case.values();
    gas.end("case values");

    assert(serialized.len() == values.len(), 'serialized not equal');
    let mut idx = 0;
    loop {
        if idx == serialized.len() {
            break;
        }
        assert(serialized.at(idx) == values.at(idx), 'serialized differ');
        idx += 1;
    };

    let layout = match dojo::model::Model::<Case>::layout() {
        Layout::Fixed(layout) => layout,
        _ => panic!("expected fixed layout"),
    };

    let gas = GasCounterImpl::start();
    database::set('cases', '42', values, 0, layout);
    gas.end("case db set");

    let gas = GasCounterImpl::start();
    database::get('cases', '42', layout);
    gas.end("case db get");
}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
#[dojo::model]
struct Character {
    #[key]
    caller: ContractAddress,
    heigth: felt252,
    abilities: Abilities,
    stats: Stats,
    weapon: Weapon,
    gold: u32,
}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
struct Abilities {
    strength: u8,
    dexterity: u8,
    constitution: u8,
    intelligence: u8,
    wisdom: u8,
    charisma: u8,
}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
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

#[derive(IntrospectPacked, Copy, Drop, Serde)]
enum Weapon {
    DualWield: (Sword, Sword),
    Fists: (Sword, Sword), // Introspect requires same arms
}

// TODO: this test should be adapted to benchmark the new layout system
#[test]
#[ignore]
#[available_gas(1000000000)]
fn bench_complex_struct_packed() {
    let gas = GasCounterImpl::start();

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
        weapon: Weapon::DualWield(
            (
                Sword {
                    swordsmith: starknet::contract_address_const::<0x69>(), damage: 0x12345678,
                },
                Sword {
                    swordsmith: starknet::contract_address_const::<0x69>(), damage: 0x12345678,
                }
            )
        ),
        gold: 0x12345678,
    };
    gas.end("chars init");

    let gas = GasCounterImpl::start();
    let mut serialized = ArrayTrait::new();
    serde::Serde::serialize(@char.heigth, ref serialized);
    serde::Serde::serialize(@char.abilities, ref serialized);
    serde::Serde::serialize(@char.stats, ref serialized);
    serde::Serde::serialize(@char.weapon, ref serialized);
    serde::Serde::serialize(@char.gold, ref serialized);
    let serialized = array::ArrayTrait::span(@serialized);
    gas.end("chars serialize");

    let gas = GasCounterImpl::start();
    let values: Span<felt252> = char.values();
    gas.end("chars values");

    assert(serialized.len() == values.len(), 'serialized not equal');

    let mut idx = 0;
    loop {
        if idx == serialized.len() {
            break;
        }
        assert(serialized.at(idx) == values.at(idx), 'serialized differ');
        idx += 1;
    };

    let layout = match dojo::model::Model::<Character>::layout() {
        Layout::Fixed(layout) => layout,
        _ => panic!("expected fixed layout"),
    };

    let gas = GasCounterImpl::start();
    database::set('chars', '42', char.values(), 0, layout);
    gas.end("chars db set");

    let gas = GasCounterImpl::start();
    database::get('chars', '42', layout);
    gas.end("chars db get");
}
