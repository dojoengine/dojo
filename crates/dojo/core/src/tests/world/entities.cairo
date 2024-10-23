use core::array::SpanTrait;

use starknet::ContractAddress;

use dojo::meta::introspect::Introspect;
use dojo::meta::Layout;
use dojo::model::{ModelIndex, Model};
use dojo::storage::database::MAX_ARRAY_LENGTH;
use dojo::utils::entity_id_from_keys;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo::tests::helpers::{
    deploy_world, deploy_world_and_bar, IbarDispatcher, IbarDispatcherTrait, Foo, foo, bar
};
use dojo::utils::test::{deploy_with_world_address, assert_array};

#[derive(Introspect, Copy, Drop, Serde)]
enum OneEnum {
    FirstArm: (u8, felt252),
    SecondArm,
}

#[derive(Introspect, Drop, Serde)]
enum AnotherEnum {
    FirstArm: (u8, OneEnum, ByteArray),
    SecondArm: (u8, OneEnum, ByteArray)
}

fn create_foo() -> Span<felt252> {
    [1, 2].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
pub struct Fizz {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
pub struct StructSimpleModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

fn create_struct_simple_model() -> Span<felt252> {
    [1, 2].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
pub struct StructWithTuple {
    #[key]
    pub caller: ContractAddress,
    pub a: (u8, u64)
}

fn create_struct_with_tuple() -> Span<felt252> {
    [12, 58].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
pub struct StructWithEnum {
    #[key]
    pub caller: ContractAddress,
    pub a: OneEnum,
}

fn create_struct_with_enum_first_variant() -> Span<felt252> {
    [0, 1, 2].span()
}

fn create_struct_with_enum_second_variant() -> Span<felt252> {
    [1].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
pub struct StructSimpleArrayModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: Array<u64>,
    pub c: u128,
}

impl ArrayU64Copy of core::traits::Copy<Array<u64>>;

fn create_struct_simple_array_model() -> Span<felt252> {
    [1, 4, 10, 20, 30, 40, 2].span()
}

#[derive(Drop, Serde)]
#[dojo::model]
pub struct StructByteArrayModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: ByteArray,
}

fn create_struct_byte_array_model() -> Span<felt252> {
    [1, 3, 'first', 'second', 'third', 'pending', 7].span()
}

#[derive(Introspect, Copy, Drop, Serde)]
pub struct ModelData {
    pub x: u256,
    pub y: u32,
    pub z: felt252
}

#[derive(Drop, Serde)]
#[dojo::model]
pub struct StructComplexArrayModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: Array<ModelData>,
    pub c: AnotherEnum,
}

fn create_struct_complex_array_model() -> Span<felt252> {
    [
        1, // a
        2, // b (array length)
        1,
        2,
        3,
        4, // item 1
        5,
        6,
        7,
        8, // item 2
        1, // c (AnotherEnum variant)
        1, // u8
        0, // OneEnum variant
        0, // u8
        123, // felt252
        1,
        'first',
        'pending',
        7 // ByteArray
    ].span()
}

#[derive(Drop, Serde)]
#[dojo::model]
pub struct StructNestedModel {
    #[key]
    pub caller: ContractAddress,
    pub x: (u8, u16, (u32, ByteArray, u8), Array<(u8, u16)>),
    pub y: Array<Array<(u8, (u16, u256))>>
}

fn create_struct_nested_model() -> Span<felt252> {
    [
        // -- x
        1, // u8
        2, // u16
        3,
        1,
        'first',
        'pending',
        7,
        9, // (u32, ByteArray, u8)
        3,
        1,
        2,
        3,
        4,
        5,
        6, // Array<(u8, u16)> with 3 items
        // -- y
        2, // Array<Array<(u8, (u16, u256))>> with 2 items
        3, // first array item - Array<(u8, (u16, u256))> of 3 items
        1,
        2,
        0,
        3, // first array item - (u8, (u16, u256))
        4,
        5,
        0,
        6, // second array item - (u8, (u16, u256))
        8,
        7,
        9,
        10, // third array item - (u8, (u16, u256))
        1, // second array item - Array<(u8, (u16, u256))> of 1 item
        5,
        4,
        6,
        7 // first array item - (u8, (u16, u256))
    ].span()
}

#[derive(Introspect, Copy, Drop, Serde)]
pub enum EnumGeneric<T, U> {
    One: T,
    Two: U
}

#[derive(Drop, Serde)]
#[dojo::model]
pub struct StructWithGeneric {
    #[key]
    pub caller: ContractAddress,
    pub x: EnumGeneric<u8, u256>,
}

fn create_struct_generic_first_variant() -> Span<felt252> {
    [0, 1].span()
}

fn create_struct_generic_second_variant() -> Span<felt252> {
    [1, 1, 2].span()
}

fn get_key_test() -> Span<felt252> {
    [0x01234].span()
}

#[test]
fn test_set_entity_admin() {
    let (world, bar_contract) = deploy_world_and_bar();

    let alice = starknet::contract_address_const::<0xa11ce>();
    starknet::testing::set_contract_address(alice);

    bar_contract.set_foo(420, 1337);

    let foo: Foo = get!(world, alice, Foo);

    println!("foo: {:?}", foo);
    assert(foo.a == 420, 'data not stored');
    assert(foo.b == 1337, 'data not stored');
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_unauthorized() {
    // Spawn empty world
    let world = deploy_world();

    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let caller = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(caller);

    // Call bar system, should panic as it's not authorized
    bar_contract.set_foo(420, 1337);
}


#[test]
fn test_set_entity_by_id() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let selector = Model::<Foo>::selector();
    let entity_id = entity_id_from_keys([0x01234].span());
    let values = create_foo();
    let layout = Model::<Foo>::layout();

    world.set_entity(selector, ModelIndex::Id(entity_id), values, layout);
    let read_values = world.entity(selector, ModelIndex::Id(entity_id), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_fixed_layout() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let selector = Model::<Foo>::selector();
    let keys = get_key_test();
    let values = create_foo();
    let layout = Model::<Foo>::layout();

    world.set_entity(selector, ModelIndex::Keys(get_key_test()), values, layout);
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleModel>::selector();
    let keys = get_key_test();
    let values = create_struct_simple_model();
    let layout = Model::<StructSimpleModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_tuple_layout() {
    let world = deploy_world();
    world.register_model(struct_with_tuple::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithTuple>::selector();
    let keys = get_key_test();
    let values = create_struct_with_tuple();
    let layout = Model::<StructWithTuple>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_enum::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithEnum>::selector();
    let keys = get_key_test();
    let values = create_struct_with_enum_first_variant();
    let layout = Model::<StructWithEnum>::layout();

    // test with the first variant
    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);

    // then override with the second variant
    let values = create_struct_with_enum_second_variant();
    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_simple_array_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_simple_array_model();
    let layout = Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_complex_array_layout() {
    let world = deploy_world();
    world.register_model(struct_complex_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructComplexArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_complex_array_model();
    let layout = Model::<StructComplexArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_layout_and_byte_array() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructByteArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_byte_array_model();
    let layout = Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_nested_elements() {
    let world = deploy_world();
    world.register_model(struct_nested_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructNestedModel>::selector();
    let keys = get_key_test();
    let values = create_struct_nested_model();
    let layout = Model::<StructNestedModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

fn assert_empty_array(values: Span<felt252>) {
    let mut i = 0;
    loop {
        if i >= values.len() {
            break;
        }
        assert!(*values.at(i) == 0);
        i += 1;
    };
}

#[test]
fn test_set_entity_with_struct_generics_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_generic::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithGeneric>::selector();
    let keys = get_key_test();
    let values = create_struct_generic_first_variant();
    let layout = Model::<StructWithGeneric>::layout();

    // test with the first variant
    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);

    // then override with the second variant
    let values = create_struct_generic_second_variant();
    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_delete_entity_by_id() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let selector = Model::<Foo>::selector();
    let entity_id = entity_id_from_keys(get_key_test());
    let values = create_foo();
    let layout = Model::<Foo>::layout();

    world.set_entity(selector, ModelIndex::Id(entity_id), values, layout);

    IWorldDispatcherTrait::delete_entity(world, selector, ModelIndex::Id(entity_id), layout);

    let read_values = world.entity(selector, ModelIndex::Id(entity_id), layout);

    assert!(read_values.len() == values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_fixed_layout() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let selector = Model::<Foo>::selector();
    let keys = get_key_test();
    let values = create_foo();
    let layout = Model::<Foo>::layout();

    world.set_entity(selector, ModelIndex::Keys(get_key_test()), values, layout);

    IWorldDispatcherTrait::delete_entity(world, selector, ModelIndex::Keys(keys), layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_simple_struct_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleModel>::selector();
    let keys = get_key_test();
    let values = create_struct_simple_model();
    let layout = Model::<StructSimpleModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    IWorldDispatcherTrait::delete_entity(world, selector, ModelIndex::Keys(keys), layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_simple_array_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_simple_array_model();
    let layout = Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    IWorldDispatcherTrait::delete_entity(world, selector, ModelIndex::Keys(keys), layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    // array length set to 0, so the expected value span is shorter than the initial values
    let expected_values = [0, 0, 0].span();

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_complex_array_struct_layout() {
    let world = deploy_world();
    world.register_model(struct_complex_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructComplexArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_complex_array_model();

    let layout = Model::<StructComplexArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    IWorldDispatcherTrait::delete_entity(world, selector, ModelIndex::Keys(keys), layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    // array length set to 0, so the expected value span is shorter than the initial values
    let expected_values = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0].span();

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_tuple_layout() {
    let world = deploy_world();
    world.register_model(struct_with_tuple::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithTuple>::selector();
    let keys = get_key_test();
    let values = create_struct_with_tuple();
    let layout = Model::<StructWithTuple>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    IWorldDispatcherTrait::delete_entity(world, selector, ModelIndex::Keys(keys), layout);

    let expected_values = [0, 0].span();
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_enum::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithEnum>::selector();
    let keys = get_key_test();
    let values = create_struct_with_enum_first_variant();
    let layout = Model::<StructWithEnum>::layout();

    // test with the first variant
    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    IWorldDispatcherTrait::delete_entity(world, selector, ModelIndex::Keys(keys), layout);

    let expected_values = [0, 0, 0].span();
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_layout_and_byte_array() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructByteArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_byte_array_model();
    let layout = Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    IWorldDispatcherTrait::delete_entity(world, selector, ModelIndex::Keys(keys), layout);

    let expected_values = [0, 0, 0, 0].span();
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_nested_elements() {
    let world = deploy_world();
    world.register_model(struct_nested_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructNestedModel>::selector();
    let keys = get_key_test();
    let values = create_struct_nested_model();
    let layout = Model::<StructNestedModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    IWorldDispatcherTrait::delete_entity(world, selector, ModelIndex::Keys(keys), layout);

    let expected_values = [0, 0, 0, 0, 0, 0, 0, 0, 0].span();
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_generics_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_generic::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithGeneric>::selector();
    let keys = get_key_test();
    let values = create_struct_generic_first_variant();
    let layout = Model::<StructWithGeneric>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    IWorldDispatcherTrait::delete_entity(world, selector, ModelIndex::Keys(keys), layout);

    let expected_values = [0, 0].span();
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_set_entity_with_unexpected_array_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Array([Introspect::<felt252>::layout()].span());

    world
        .set_entity(
            Model::<StructSimpleArrayModel>::selector(),
            ModelIndex::Keys([].span()),
            [].span(),
            layout
        );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_set_entity_with_unexpected_tuple_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Tuple([Introspect::<felt252>::layout()].span());

    world
        .set_entity(
            Model::<StructSimpleArrayModel>::selector(),
            ModelIndex::Keys([].span()),
            [].span(),
            layout
        );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_delete_entity_with_unexpected_array_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Array([Introspect::<felt252>::layout()].span());
    IWorldDispatcherTrait::delete_entity(
        world, Model::<StructSimpleArrayModel>::selector(), ModelIndex::Keys([].span()), layout
    );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_delete_entity_with_unexpected_tuple_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Tuple([Introspect::<felt252>::layout()].span());

    IWorldDispatcherTrait::delete_entity(
        world, Model::<StructSimpleArrayModel>::selector(), ModelIndex::Keys([].span()), layout
    );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_get_entity_with_unexpected_array_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Array([Introspect::<felt252>::layout()].span());

    world.entity(Model::<StructSimpleArrayModel>::selector(), ModelIndex::Keys([].span()), layout);
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_get_entity_with_unexpected_tuple_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Tuple([Introspect::<felt252>::layout()].span());

    world.entity(Model::<StructSimpleArrayModel>::selector(), ModelIndex::Keys([].span()), layout);
}


#[test]
#[should_panic(expected: ('Invalid values length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_bad_values_length_error_for_array_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleArrayModel>::selector();
    let keys = get_key_test();
    let layout = Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), [1].span(), layout);
}

#[test]
#[should_panic(expected: ('invalid array length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_too_big_array_length() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleArrayModel>::selector();
    let keys = get_key_test();
    let values: Span<felt252> = [
        1, MAX_ARRAY_LENGTH.try_into().unwrap() + 1, 10, 20, 30, 40, 2
    ].span();
    let layout = Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);
}

#[test]
#[should_panic(expected: ('invalid array length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_struct_layout_and_bad_byte_array_length() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructByteArrayModel>::selector();
    let keys = get_key_test();
    let values: Span<felt252> = [
        1, MAX_ARRAY_LENGTH.try_into().unwrap(), 'first', 'second', 'third', 'pending', 7
    ].span();
    let layout = Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);
}

#[test]
#[should_panic(expected: ('Invalid values length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_struct_layout_and_bad_value_length_for_byte_array() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructByteArrayModel>::selector();
    let keys = get_key_test();
    let values: Span<felt252> = [1, 3, 'first', 'second', 'third', 'pending'].span();
    let layout = Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);
}

fn write_foo_record(world: IWorldDispatcher) {
    let selector = Model::<Foo>::selector();
    let values = create_foo();
    let layout = Model::<Foo>::layout();

    world.set_entity(selector, ModelIndex::Keys(get_key_test()), values, layout);
}
