use dojo::model::{Model, ModelValue, ModelStorage, ModelValueStorage, ModelPtr};
use dojo::world::WorldStorage;
use dojo_cairo_test::{spawn_test_world, NamespaceDef, TestResource};

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
#[dojo::model]
struct Foo {
    #[key]
    k1: u8,
    #[key]
    k2: felt252,
    v1: u128,
    v2: u32,
}


#[derive(Copy, Drop, Serde, Debug)]
#[dojo::model]
struct Foo2 {
    #[key]
    k1: u8,
    #[key]
    k2: felt252,
    v1: u128,
    v2: u32,
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::model]
struct Foo3 {
    #[key]
    k1: u256,
    #[key]
    k2: felt252,
    v1: u128,
    v2: u32,
}

#[derive(Copy, Drop, Serde, Debug, Introspect)]
struct AStruct {
    a: u8,
    b: u8,
    c: u8,
    d: u8,
}

#[dojo::model]
#[derive(Copy, Drop, Serde, Debug)]
struct Foo4 {
    #[key]
    id: felt252,
    v0: u256,
    v1: felt252,
    v2: u128,
    v3: AStruct,
}

#[derive(Copy, Drop, Serde, Debug, Introspect)]
struct FooSchema {
    v0: u256,
    v3: AStruct,
}

// to test the issue https://github.com/dojoengine/dojo/issues/3199
// see `extract_composite_inner_type` function in dojo-lang.
#[derive(Copy, Drop, Serde, Debug)]
#[dojo::model]
struct ModelWithCommentOnLastFied {
    #[key]
    k1: u8,
    v1: Span<u32> // a comment without a comma 
}

// to test with unit types
#[derive(Copy, Drop, Introspect, Debug, Serde, PartialEq)]
enum EnumWithUnitType {
    X: u8,
    Y,
    Z: (),
}

#[derive(Copy, Drop, Introspect, Debug, Serde, PartialEq)]
struct StructWithUnitType {
    x: (),
}

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
#[dojo::model]
struct ModelWithUnitType {
    #[key]
    k: u8,
    x: StructWithUnitType,
    y: EnumWithUnitType,
    z: (),
    a: ((), (u8, ())),
}

fn namespace_def() -> NamespaceDef {
    NamespaceDef {
        namespace: "dojo_cairo_test",
        resources: [
            TestResource::Model(m_Foo::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Model(m_Foo2::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Model(m_Foo3::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Model(m_Foo4::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Model(m_ModelWithUnitType::TEST_CLASS_HASH.try_into().unwrap()),
        ]
            .span(),
    }
}

fn spawn_foo_world() -> WorldStorage {
    spawn_test_world([namespace_def()].span())
}

#[test]
fn test_model_definition() {
    let definition = dojo::model::Model::<Foo>::definition();

    assert_eq!(definition.name, dojo::model::Model::<Foo>::name());
    assert_eq!(definition.layout, dojo::model::Model::<Foo>::layout());
    assert_eq!(definition.schema, dojo::model::Model::<Foo>::schema());
    assert_eq!(definition.packed_size, dojo::model::Model::<Foo>::packed_size());
    assert_eq!(definition.unpacked_size, dojo::meta::introspect::Introspect::<Foo>::size());
}

#[test]
fn test_values() {
    let mvalues = FooValue { v1: 3, v2: 4 };
    let expected_values = [3, 4].span();

    let values = mvalues.serialized_values();
    assert!(expected_values == values);
}

#[test]
fn test_from_values() {
    let mut values = [3, 4].span();

    let model_values: Option<FooValue> = ModelValue::<FooValue>::from_serialized(values);
    assert!(model_values.is_some());
    let model_values = model_values.unwrap();
    assert!(model_values.v1 == 3 && model_values.v2 == 4);
}

#[test]
fn test_from_values_bad_data() {
    let mut values = [3].span();
    let res: Option<FooValue> = ModelValue::<FooValue>::from_serialized(values);
    assert!(res.is_none());
}

#[test]
fn test_read_and_update_model_value() {
    let mut world = spawn_foo_world();

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);

    let entity_id = foo.entity_id();
    let mut model_value: FooValue = world.read_value(foo.keys());
    assert_eq!(model_value.v1, foo.v1);
    assert_eq!(model_value.v2, foo.v2);

    model_value.v1 = 12;
    model_value.v2 = 18;

    world.write_value_from_id(entity_id, @model_value);

    let read_values: FooValue = world.read_value(foo.keys());
    assert!(read_values.v1 == model_value.v1 && read_values.v2 == model_value.v2);
}

#[test]
fn test_delete_model_value() {
    let mut world = spawn_foo_world();

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);

    let entity_id = foo.entity_id();
    ModelStorage::<WorldStorage, Foo>::erase_model(ref world, @foo);

    let read_values: FooValue = world.read_value_from_id(entity_id);
    assert!(read_values.v1 == 0 && read_values.v2 == 0);
}

#[test]
fn test_read_and_write_field_name() {
    let mut world = spawn_foo_world();

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);

    // Inference fails here, we need something better without too generics
    // which also fails.
    let v1 = world.read_member(foo.ptr(), selector!("v1"));
    assert!(foo.v1 == v1);

    world.write_member(foo.ptr(), selector!("v1"), 42);

    let v1 = world.read_member(foo.ptr(), selector!("v1"));
    assert!(v1 == 42);
}

#[test]
fn test_read_and_write_from_model() {
    let mut world = spawn_foo_world();

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);

    let foo2: Foo = world.read_model((foo.k1, foo.k2));

    assert!(foo.k1 == foo2.k1 && foo.k2 == foo2.k2 && foo.v1 == foo2.v1 && foo.v2 == foo2.v2);
}

#[test]
fn test_delete_from_model() {
    let mut world = spawn_foo_world();

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);
    world.erase_model(@foo);

    let foo2: Foo = world.read_model((foo.k1, foo.k2));
    assert!(foo2.k1 == foo.k1 && foo2.k2 == foo.k2 && foo2.v1 == 0 && foo2.v2 == 0);
}

#[test]
fn test_model_ptr_from_keys() {
    let mut world = spawn_foo_world();
    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    let ptr = Model::<Foo>::ptr_from_keys(foo.keys());
    world.write_model(@foo);
    let v1 = world.read_member(ptr, selector!("v1"));
    assert!(foo.v1 == v1);
}

#[test]
fn test_model_ptr_from_serialized_keys() {
    let mut world = spawn_foo_world();
    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    let ptr = Model::<Foo>::ptr_from_serialized_keys(foo.serialized_keys());
    world.write_model(@foo);
    let v1 = world.read_member(ptr, selector!("v1"));
    assert!(foo.v1 == v1);
}

#[test]
fn test_model_ptr_from_entity_id() {
    let mut world = spawn_foo_world();
    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    let ptr = Model::<Foo>::ptr_from_id(foo.entity_id());
    world.write_model(@foo);
    let v1 = world.read_member(ptr, selector!("v1"));
    assert!(foo.v1 == v1);
}

#[test]
fn test_read_member() {
    let mut world = spawn_foo_world();
    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);
    let v1: u128 = world.read_member(foo.ptr(), selector!("v1"));
    let v2: u32 = world.read_member(foo.ptr(), selector!("v2"));
    assert!(foo.v1 == v1);
    assert!(foo.v2 == v2);
}

#[test]
fn test_read_members() {
    let mut world = spawn_foo_world();
    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    let foo2 = Foo { k1: 5, k2: 6, v1: 7, v2: 8 };
    world.write_models([@foo, @foo2].span());
    let ptrs = [foo.ptr(), foo2.ptr()].span();
    let v1s: Array<u128> = world.read_member_of_models(ptrs, selector!("v1"));
    let v2s: Array<u32> = world.read_member_of_models(ptrs, selector!("v2"));
    assert!(v1s == array![foo.v1, foo2.v1]);
    assert!(v2s == array![foo.v2, foo2.v2]);
}

#[test]
fn test_write_member() {
    let mut world = spawn_foo_world();
    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);
    world.write_member(foo.ptr(), selector!("v1"), 42);
    let foo_read: Foo = world.read_model((foo.k1, foo.k2));
    assert!(foo_read.v1 == 42 && foo_read.v2 == foo.v2);
}
#[test]
fn test_write_members() {
    let mut world = spawn_foo_world();
    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    let foo2 = Foo { k1: 5, k2: 6, v1: 7, v2: 8 };
    world.write_models([@foo, @foo2].span());
    let ptrs = [foo.ptr(), foo2.ptr()].span();
    let v1s = array![42, 43];
    world.write_member_of_models(ptrs, selector!("v1"), v1s.span());
    let v1s_read: Array<u128> = world.read_member_of_models(ptrs, selector!("v1"));
    let v2s_read: Array<u32> = world.read_member_of_models(ptrs, selector!("v2"));
    assert!(v1s_read == v1s);
    assert!(v2s_read == array![foo.v2, foo2.v2]);
}

#[test]
fn test_ptr_from() {
    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    let ptr_a = ModelPtr::<Foo> { id: foo.entity_id() };
    let ptr_b = Model::<Foo>::ptr_from_keys(foo.keys());
    let ptr_c = Model::<Foo>::ptr_from_serialized_keys([foo.k1.into(), foo.k2].span());
    let ptr_d = Model::<Foo>::ptr_from_id(foo.entity_id());
    assert!(ptr_a == ptr_b && ptr_a == ptr_c && ptr_a == ptr_d);
}

#[test]
fn test_ptrs_from() {
    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    let foo2 = Foo { k1: 3, k2: 4, v1: 5, v2: 6 };
    let ptrs_a = [ModelPtr::<Foo> { id: foo.entity_id() }, ModelPtr::<Foo> { id: foo2.entity_id() }]
        .span();
    let ptrs_b = Model::<Foo>::ptrs_from_keys([foo.keys(), foo2.keys()].span());
    let ptrs_c = Model::<
        Foo,
    >::ptrs_from_serialized_keys(
        [[foo.k1.into(), foo.k2].span(), [foo2.k1.into(), foo2.k2].span()].span(),
    );
    let ptrs_d = Model::<Foo>::ptrs_from_ids([foo.entity_id(), foo2.entity_id()].span());
    assert!(ptrs_a == ptrs_b && ptrs_a == ptrs_c && ptrs_a == ptrs_d);
}

#[test]
fn test_read_schema() {
    let mut world = spawn_foo_world();
    let foo = Foo4 { id: 1, v0: 2, v1: 3, v2: 4, v3: AStruct { a: 5, b: 6, c: 7, d: 8 } };
    world.write_model(@foo);

    let schema: FooSchema = world.read_schema(foo.ptr());
    assert!(
        schema.v0 == foo.v0
            && schema.v3.a == foo.v3.a
            && schema.v3.b == foo.v3.b
            && schema.v3.c == foo.v3.c
            && schema.v3.d == foo.v3.d,
    );
}

#[test]
fn test_read_schemas() {
    let mut world = spawn_foo_world();
    let foo = Foo4 { id: 1, v0: 2, v1: 3, v2: 4, v3: AStruct { a: 5, b: 6, c: 7, d: 8 } };
    let mut foo_2 = foo;
    foo_2.id = 2;
    foo_2.v0 = 12;

    world.write_models([@foo, @foo_2].span());

    let mut values: Array<FooSchema> = world.read_schemas([foo.ptr(), foo_2.ptr()].span());
    let schema_1 = values.pop_front().unwrap();
    let schema_2 = values.pop_front().unwrap();
    assert!(
        schema_1.v0 == foo.v0
            && schema_1.v3.a == foo.v3.a
            && schema_1.v3.b == foo.v3.b
            && schema_1.v3.c == foo.v3.c
            && schema_1.v3.d == foo.v3.d,
    );
    assert!(
        schema_2.v0 == foo_2.v0
            && schema_2.v3.a == foo_2.v3.a
            && schema_2.v3.b == foo_2.v3.b
            && schema_2.v3.c == foo_2.v3.c
            && schema_2.v3.d == foo_2.v3.d,
    );
}

#[test]
fn test_access_with_unit_type() {
    let mut world = spawn_foo_world();

    let m = ModelWithUnitType {
        k: 1, x: StructWithUnitType { x: () }, y: EnumWithUnitType::Z(()), z: (), a: ((), (12, ())),
    };

    world.write_model(@m);

    let read_m: ModelWithUnitType = world.read_model(1);
    let read_default_m: ModelWithUnitType = world.read_model(2);

    assert!(m == read_m, "Bad read model");
    assert!(
        read_default_m == ModelWithUnitType {
            k: 2,
            x: StructWithUnitType { x: () },
            y: EnumWithUnitType::X(0),
            z: (),
            a: ((), (0, ())),
        },
        "Bad default model",
    );
}
