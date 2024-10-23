use core::array::{ArrayTrait, SpanTrait};
use core::poseidon::poseidon_hash_span;
use core::result::ResultTrait;
use core::serde::Serde;

use starknet::{ContractAddress, SyscallResultTrait};
use starknet::storage_access::{
    storage_base_address_from_felt252, storage_address_from_base,
    storage_address_from_base_and_offset
};
use starknet::syscalls::{storage_read_syscall, storage_write_syscall};

use dojo::meta::Layout;
use dojo::model::{Model, ModelIndex, ModelStore};
use dojo::storage::{database, storage};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo::tests::helpers::{Foo, Sword, Case, CaseStore, case, Character, Abilities, Stats, Weapon};
use dojo::utils::test::{spawn_test_world, GasCounterTrait};

#[derive(Drop, Serde)]
#[dojo::model]
struct CaseNotPacked {
    #[key]
    pub owner: ContractAddress,
    pub sword: Sword,
    pub material: felt252,
}

#[derive(Drop, Serde)]
#[dojo::model]
struct ComplexModel {
    #[key]
    pub game_id: u128,
    #[key]
    pub player: ContractAddress,
    pub first_name: ByteArray,
    pub last_name: ByteArray,
    pub weapons: Array<Weapon>,
    pub abilities: (Abilities, Abilities),
    pub stats: Stats,
}

fn deploy_world() -> IWorldDispatcher {
    spawn_test_world(
        ["dojo"].span(),
        [
            case::TEST_CLASS_HASH, case_not_packed::TEST_CLASS_HASH, complex_model::TEST_CLASS_HASH
        ].span()
    )
}

#[test]
#[available_gas(1000000000)]
fn bench_reference_offset() {
    let gas = GasCounterTrait::start();
    gas.end("bench empty");
}

#[test]
#[available_gas(1000000000)]
fn bench_storage_single() {
    let keys = ['database_test', '42'].span();

    let gas = GasCounterTrait::start();
    storage::set(0, keys, 420);
    gas.end("storage set");

    let gas = GasCounterTrait::start();
    let res = storage::get(0, keys);
    gas.end("storage get");

    assert(res == 420, 'values differ');
}

#[test]
#[available_gas(1000000000)]
fn bench_storage_many() {
    let keys = [0x1337].span();
    let values = [1, 2].span();
    let layout = [251, 251].span();

    let gas = GasCounterTrait::start();
    storage::set_many(0, keys, values, 0, layout).unwrap();
    gas.end("storage set_many");

    let gas = GasCounterTrait::start();
    let res = storage::get_many(0, keys, layout).unwrap();
    gas.end("storage get_many");

    assert(res.len() == 2, 'wrong number of values');
    assert(*res.at(0) == *values.at(0), 'value not set');
    assert(*res.at(1) == *values.at(1), 'value not set');
}

#[test]
#[available_gas(1000000000)]
fn bench_native_storage() {
    let gas = GasCounterTrait::start();
    let keys = [0x1337].span();
    let base = storage_base_address_from_felt252(poseidon_hash_span(keys));
    let address = storage_address_from_base(base);
    gas.end("native prep");

    let gas = GasCounterTrait::start();
    storage_write_syscall(0, address, 42).unwrap_syscall();
    gas.end("native write");

    let gas = GasCounterTrait::start();
    let value = storage_read_syscall(0, address).unwrap_syscall();
    gas.end("native read");

    assert(value == 42, 'read invalid');
}

#[test]
#[available_gas(1000000000)]
fn bench_native_storage_offset() {
    let gas = GasCounterTrait::start();
    let keys = [0x1337].span();
    let base = storage_base_address_from_felt252(poseidon_hash_span(keys));
    let address = storage_address_from_base_and_offset(base, 42);
    gas.end("native prep of");

    let gas = GasCounterTrait::start();
    storage_write_syscall(0, address, 42).unwrap_syscall();
    gas.end("native writ of");

    let gas = GasCounterTrait::start();
    let value = storage_read_syscall(0, address).unwrap_syscall();
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

    let gas = GasCounterTrait::start();
    database::set('table', 'key', values.span(), 0, layout.span());
    gas.end("db set arr");

    let gas = GasCounterTrait::start();
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

    let gas = GasCounterTrait::start();
    let mut foo = Foo { caller, a: 0x123456789abcdef, b: 0x123456789abcdef, };
    gas.end("foo init");

    let gas = GasCounterTrait::start();
    let mut serialized = ArrayTrait::new();
    Serde::serialize(@foo.a, ref serialized);
    Serde::serialize(@foo.b, ref serialized);
    let serialized = ArrayTrait::span(@serialized);
    gas.end("foo serialize");

    let gas = GasCounterTrait::start();
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
    let gas = GasCounterTrait::start();

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

    let gas = GasCounterTrait::start();
    let mut serialized = ArrayTrait::new();
    Serde::serialize(@pos.x, ref serialized);
    Serde::serialize(@pos.y, ref serialized);
    Serde::serialize(@pos.z, ref serialized);
    Serde::serialize(@pos.a, ref serialized);
    Serde::serialize(@pos.b, ref serialized);
    Serde::serialize(@pos.c, ref serialized);
    Serde::serialize(@pos.d, ref serialized);
    let serialized = ArrayTrait::span(@serialized);
    gas.end("pos serialize");

    let gas = GasCounterTrait::start();
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

    let gas = GasCounterTrait::start();
    database::set('positions', '42', pos.values(), 0, layout);
    gas.end("pos db set");

    let gas = GasCounterTrait::start();
    database::get('positions', '42', layout);
    gas.end("pos db get");
}

// TODO: this test should be adapted to benchmark the new layout system
#[test]
#[ignore]
#[available_gas(1000000000)]
fn bench_nested_struct_packed() {
    let caller = starknet::contract_address_const::<0x42>();

    let gas = GasCounterTrait::start();
    let mut case = Case {
        owner: caller, sword: Sword { swordsmith: caller, damage: 0x12345678, }, material: 'wooden',
    };
    gas.end("case init");

    let gas = GasCounterTrait::start();
    let mut serialized = ArrayTrait::new();
    Serde::serialize(@case.sword, ref serialized);
    Serde::serialize(@case.material, ref serialized);
    let serialized = ArrayTrait::span(@serialized);
    gas.end("case serialize");

    let gas = GasCounterTrait::start();
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

    let gas = GasCounterTrait::start();
    database::set('cases', '42', values, 0, layout);
    gas.end("case db set");

    let gas = GasCounterTrait::start();
    database::get('cases', '42', layout);
    gas.end("case db get");
}

// TODO: this test should be adapted to benchmark the new layout system
#[test]
#[ignore]
#[available_gas(1000000000)]
fn bench_complex_struct_packed() {
    let gas = GasCounterTrait::start();

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

    let gas = GasCounterTrait::start();
    let mut serialized = ArrayTrait::new();
    Serde::serialize(@char.heigth, ref serialized);
    Serde::serialize(@char.abilities, ref serialized);
    Serde::serialize(@char.stats, ref serialized);
    Serde::serialize(@char.weapon, ref serialized);
    Serde::serialize(@char.gold, ref serialized);
    let serialized = ArrayTrait::span(@serialized);
    gas.end("chars serialize");

    let gas = GasCounterTrait::start();
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

    let gas = GasCounterTrait::start();
    database::set('chars', '42', char.values(), 0, layout);
    gas.end("chars db set");

    let gas = GasCounterTrait::start();
    database::get('chars', '42', layout);
    gas.end("chars db get");
}

#[test]
fn test_benchmark_set_entity() {
    let world = deploy_world();
    let bob = starknet::contract_address_const::<0xb0b>();

    let simple_entity_packed = Case {
        owner: bob, sword: Sword { swordsmith: bob, damage: 42, }, material: 'iron'
    };

    let simple_entity_not_packed = CaseNotPacked {
        owner: bob, sword: Sword { swordsmith: bob, damage: 42, }, material: 'iron'
    };

    let complex_entity = ComplexModel {
        game_id: 0x123456789ABCDEF123456789ABCDEF,
        player: bob,
        first_name: "John",
        last_name: "Doe",
        weapons: array![
            Weapon::DualWield(
                (Sword { swordsmith: bob, damage: 42 }, Sword { swordsmith: bob, damage: 800 })
            ),
            Weapon::Fists(
                (Sword { swordsmith: bob, damage: 300 }, Sword { swordsmith: bob, damage: 1200 })
            )
        ],
        abilities: (
            Abilities {
                strength: 12,
                dexterity: 32,
                constitution: 8,
                intelligence: 14,
                wisdom: 1,
                charisma: 25,
            },
            Abilities {
                strength: 28,
                dexterity: 13,
                constitution: 18,
                intelligence: 2,
                wisdom: 1,
                charisma: 43,
            }
        ),
        stats: Stats {
            kills: 99,
            deaths: 1,
            rests: 23,
            hits: 8192,
            blocks: 4301,
            walked: 12,
            runned: 32,
            finished: true,
            romances: 65
        },
    };

    let gas = GasCounterTrait::start();
    world
        .set_entity(
            model_selector: Model::<Case>::selector(),
            index: ModelIndex::Keys(simple_entity_packed.keys()),
            values: simple_entity_packed.values(),
            layout: Model::<Case>::layout()
        );
    gas.end("World::SetEntity::SimplePacked");

    let gas = GasCounterTrait::start();
    world
        .set_entity(
            model_selector: Model::<CaseNotPacked>::selector(),
            index: ModelIndex::Keys(simple_entity_not_packed.keys()),
            values: simple_entity_not_packed.values(),
            layout: Model::<CaseNotPacked>::layout()
        );
    gas.end("World::SetEntity::SimpleNotPacked");

    let gas = GasCounterTrait::start();
    world
        .set_entity(
            model_selector: Model::<ComplexModel>::selector(),
            index: ModelIndex::Keys(complex_entity.keys()),
            values: complex_entity.values(),
            layout: Model::<ComplexModel>::layout()
        );
    gas.end("World::SetEntity::ComplexModel");

    let gas = GasCounterTrait::start();
    world.set(@simple_entity_packed);
    gas.end("Model::Set::SimplePacked");

    let gas = GasCounterTrait::start();
    world.set(@simple_entity_not_packed);
    gas.end("Model::Set::SimpleNotPacked");

    let gas = GasCounterTrait::start();
    world.set(@complex_entity);
    gas.end("Model::Set::ComplexModel");

    let gas = GasCounterTrait::start();
    set!(world, (simple_entity_packed,));
    gas.end("Macro::Set::SimplePacked");

    let gas = GasCounterTrait::start();
    set!(world, (simple_entity_not_packed,));
    gas.end("Macro::Set::SimpleNotPacked");

    let gas = GasCounterTrait::start();
    set!(world, (complex_entity,));
    gas.end("Macro::Set::ComplexModel");
}

