use dojo::meta::{FieldLayout, Introspect, Layout};
use dojo::model::{
    Model, ModelPtr, ModelStorage, ModelStorageTest, ModelValue, ModelValueStorage,
    ModelValueStorageTest,
};
use dojo::world::WorldStorage;
use dojo_snf_test::world::{NamespaceDef, TestResource, spawn_test_world};

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

#[derive(Copy, Drop, Serde, Debug, Introspect, DojoStore)]
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

#[derive(Copy, Drop, Serde, Debug, Introspect, DojoStore)]
struct FooSchema {
    v0: u256,
    v3: AStruct,
}

// to test the issue https://github.com/dojoengine/dojo/issues/3199
#[derive(Copy, Drop, Serde, Debug)]
#[dojo::model]
struct ModelWithCommentOnLastFied {
    #[key]
    k1: u8,
    v1: Span<u32> // a comment without a comma 
}

#[derive(Copy, Drop, Serde, Debug, Introspect, Default, PartialEq)]
enum EnumWithCommentOnLastVariant {
    #[default]
    X: u8,
    Y: Span<u32> // a comment without a comma
}

#[derive(Copy, Drop, Serde, Debug, Introspect, Default, PartialEq)]
enum MyEnumLegacy {
    X: Option<u32>,
    Y: (u8, u32),
    #[default]
    Z,
}

#[derive(Copy, Drop, Serde, Debug, DojoLegacyStore, PartialEq)]
#[dojo::model]
struct LegacyModel {
    #[key]
    a: u8,
    b: (u8, u32),
    c: Option<u32>,
    d: MyEnumLegacy,
}


#[derive(Copy, Drop, Serde, Debug, Introspect, Default, PartialEq, DojoStore)]
enum MyEnum {
    X: Option<u32>,
    Y: (u8, u32),
    #[default]
    Z,
}

#[derive(Copy, Drop, Serde, Debug, Introspect, PartialEq)]
#[dojo::model]
struct DojoStoreModel {
    #[key]
    a: u8,
    b: (u8, u32),
    c: Option<u32>,
    d: MyEnum,
}

#[derive(Copy, Drop, Serde, Introspect, Default, Debug, PartialEq)]
enum EnumKey {
    #[default]
    KEY_1,
    KEY_2,
    KEY_3,
}

#[derive(Copy, Drop, Debug, DojoLegacyStore, PartialEq)]
#[dojo::model]
struct LegacyModelWithEnumKey {
    #[key]
    k1: u8,
    #[key]
    k2: EnumKey,
    v1: u32,
    v2: Option<u32>,
    v3: MyEnumLegacy,
}

#[derive(Copy, Drop, Serde, Introspect, Debug, PartialEq)]
struct LegacyModelSubset {
    v2: Option<u32>,
    v3: MyEnumLegacy,
}

#[derive(Copy, Drop, Debug, PartialEq)]
#[dojo::model]
struct DojoStoreModelWithEnumKey {
    #[key]
    k1: u8,
    #[key]
    k2: EnumKey,
    v1: u32,
    v2: Option<u32>,
    v3: MyEnum,
}

#[derive(Copy, Drop, Serde, Introspect, DojoStore, Debug, PartialEq)]
struct DojoStoreModelSubset {
    v2: Option<u32>,
    v3: MyEnum,
}

// to test with unit types
#[derive(Copy, Drop, Introspect, Debug, Serde, PartialEq, Default, DojoStore)]
enum EnumWithUnitType {
    #[default]
    X: u8,
    Y,
    Z: (),
}

#[derive(Copy, Drop, Introspect, Debug, Serde, PartialEq, DojoStore)]
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

// To test DojoStore impls for tuples
#[derive(Introspect, Serde, Drop, Default)]
struct StructWithTuples {
    x: (u8, u16, u32),
    y: Array<(u128, u128)>,
    z: (u8, (u16, Option<u32>), (), u32),
}

// To test DojoStore impls for tuples
#[derive(Introspect, Serde, Drop, Default)]
enum EnumWithTuples {
    #[default]
    A: (u8, u16, u32),
    B: Array<(u128, u128)>,
    C: (u8, (u16, Option<u32>), (), u32),
}

// To test DojoStore impls for tuples
#[derive(IntrospectPacked, Serde, Drop, Default)]
struct StructPackedWithTuples {
    x: (u8, u16, u32),
    y: (u8, (u16, u32), (), u32),
}

// To test DojoStore impls for tuples
#[derive(IntrospectPacked, Serde, Drop, Default)]
enum EnumPackedWithTuples {
    #[default]
    A: (u8, (u16, u32), (), u32),
    B: (u8, (u16, u32), (), u32),
}

// To test Option with tuple
#[derive(PartialEq)]
#[dojo::model]
struct StructWithOptionWithTuple {
    #[key]
    k: u8,
    x: Option<(u8, u16)>,
    y: Option<u32>,
}

#[dojo::model]
struct ModelWithFixedArray {
    #[key]
    k1: u8,
    v1: [u16; 3],
}

fn namespace_def() -> NamespaceDef {
    NamespaceDef {
        namespace: "dojo_core_test",
        resources: [
            TestResource::Model("Foo"), TestResource::Model("Foo2"), TestResource::Model("Foo3"),
            TestResource::Model("Foo4"), TestResource::Model("ModelWithUnitType"),
            TestResource::Model("LegacyModel"), TestResource::Model("DojoStoreModel"),
            TestResource::Model("LegacyModelWithEnumKey"),
            TestResource::Model("DojoStoreModelWithEnumKey"),
            TestResource::Model("StructWithOptionWithTuple"),
            TestResource::Model("ModelWithFixedArray"),
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
    )
}

#[test]
fn test_struct_with_option_with_tuple() {
    let mut world = spawn_foo_world();

    let m = StructWithOptionWithTuple { k: 1, x: Option::Some((1, 2)), y: Option::Some(3) };
    world.write_model(@m);

    let read_m: StructWithOptionWithTuple = world.read_model(1);

    assert!(m == read_m, "Bad model with Option with tuple");
}

#[test]
fn test_model_with_fixed_array() {
    let mut world = spawn_foo_world();
    let model = ModelWithFixedArray { k1: 1, v1: [4, 32, 256] };

    world.write_model(@model);
    let read_model: ModelWithFixedArray = world.read_model(model.keys());

    assert!(model.v1 == read_model.v1);

    world.erase_model(@model);
    let read_model: ModelWithFixedArray = world.read_model(model.keys());

    assert!(read_model.v1 == [0, 0, 0]);
}

#[test]
fn test_legacy_model() {
    let mut _world = spawn_foo_world();

    // legacy layout
    let definition = dojo::model::Model::<LegacyModel>::definition();
    let layout = Introspect::<LegacyModel>::layout();

    let expected_layout = Layout::Struct(
        [
            FieldLayout {
                selector: selector!("b"),
                layout: Layout::Tuple(
                    [Introspect::<u8>::layout(), Introspect::<u32>::layout()].span(),
                ),
            },
            FieldLayout {
                selector: selector!("c"),
                layout: Layout::Enum(
                    [
                        FieldLayout { selector: 0, layout: Introspect::<u32>::layout() },
                        FieldLayout { selector: 1, layout: Layout::Fixed([].span()) },
                    ]
                        .span(),
                ),
            },
            FieldLayout {
                selector: selector!("d"),
                layout: Layout::Enum(
                    [
                        FieldLayout {
                            selector: 0,
                            layout: Layout::Enum(
                                [
                                    FieldLayout {
                                        selector: 0, layout: Introspect::<u32>::layout(),
                                    },
                                    FieldLayout { selector: 1, layout: Layout::Fixed([].span()) },
                                ]
                                    .span(),
                            ),
                        },
                        FieldLayout {
                            selector: 1,
                            layout: Layout::Tuple(
                                [Introspect::<u8>::layout(), Introspect::<u32>::layout()].span(),
                            ),
                        },
                        FieldLayout { selector: 2, layout: Layout::Fixed([].span()) },
                    ]
                        .span(),
                ),
            },
        ]
            .span(),
    );

    // the layout returned by Introspect::layout() is the layout of the model
    // for the new storage system, while the layout returned by ModelDefinition
    // is the layout used to store data (so it is adapted in case of legacy storage model).
    // This is fine as Introspect::layout() is never used for models.
    assert_eq!(definition.layout, expected_layout, "ModelDefinition: bad layout");
    assert_ne!(layout, expected_layout, "Introspect::layout(): bad layout");

    // (de)serialization
    let m = LegacyModel {
        a: 42, b: (83, 1234), c: Option::Some(987), d: MyEnumLegacy::X(Option::Some(5432)),
    };
    let serialized_keys = dojo::model::model::ModelParser::<LegacyModel>::serialize_keys(@m);
    let serialized_values = dojo::model::model::ModelParser::<LegacyModel>::serialize_values(@m);

    assert_eq!(serialized_keys, [42].span(), "LegacyModel: serialize_keys failed");
    assert_eq!(
        serialized_values,
        [83, 1234, 0, 987, 0, 0, 5432].span(),
        "LegacyModel: serialize_values failed",
    );

    let mut keys = [42].span();
    let mut values = [83, 1234, 0, 987, 0, 0, 5432].span();

    assert_eq!(
        dojo::model::model::ModelParser::<LegacyModel>::deserialize(ref keys, ref values),
        Option::Some(m),
        "LegacyModel: deserialize failed",
    );
}

#[test]
fn test_dojo_store_model() {
    let mut _world = spawn_foo_world();

    // DojoStore layout
    let definition = dojo::model::Model::<DojoStoreModel>::definition();
    let layout = Introspect::<DojoStoreModel>::layout();

    let expected_layout = Layout::Struct(
        [
            FieldLayout {
                selector: selector!("b"),
                layout: Layout::Tuple(
                    [Introspect::<u8>::layout(), Introspect::<u32>::layout()].span(),
                ),
            },
            FieldLayout {
                selector: selector!("c"),
                layout: Layout::Enum(
                    [
                        FieldLayout { selector: 1, layout: Introspect::<u32>::layout() },
                        FieldLayout { selector: 2, layout: Layout::Fixed([].span()) },
                    ]
                        .span(),
                ),
            },
            FieldLayout {
                selector: selector!("d"),
                layout: Layout::Enum(
                    [
                        FieldLayout {
                            selector: 1,
                            layout: Layout::Enum(
                                [
                                    FieldLayout {
                                        selector: 1, layout: Introspect::<u32>::layout(),
                                    },
                                    FieldLayout { selector: 2, layout: Layout::Fixed([].span()) },
                                ]
                                    .span(),
                            ),
                        },
                        FieldLayout {
                            selector: 2,
                            layout: Layout::Tuple(
                                [Introspect::<u8>::layout(), Introspect::<u32>::layout()].span(),
                            ),
                        },
                        FieldLayout { selector: 3, layout: Layout::Fixed([].span()) },
                    ]
                        .span(),
                ),
            },
        ]
            .span(),
    );

    assert_eq!(definition.layout, expected_layout, "ModelDefinition: bad layout");
    assert_eq!(layout, expected_layout, "Introspect::layout(): bad layout");

    // (de)serialization
    let m = DojoStoreModel {
        a: 42, b: (83, 1234), c: Option::Some(987), d: MyEnum::X(Option::Some(5432)),
    };
    let serialized_keys = dojo::model::model::ModelParser::<DojoStoreModel>::serialize_keys(@m);
    let serialized_values = dojo::model::model::ModelParser::<DojoStoreModel>::serialize_values(@m);

    assert_eq!(serialized_keys, [42].span(), "DojoStoreModel: serialize_keys failed");
    assert_eq!(
        serialized_values,
        [83, 1234, 1, 987, 1, 1, 5432].span(),
        "DojoStoreModel: serialize_values failed",
    );

    let mut keys = [42].span();
    let mut values = [83, 1234, 1, 987, 1, 1, 5432].span();

    assert_eq!(
        dojo::model::model::ModelParser::<DojoStoreModel>::deserialize(ref keys, ref values),
        Option::Some(m),
        "DojoStoreModel: deserialize failed",
    );
}

#[test]
fn test_legacy_model_with_enum_key_serialization() {
    let m = LegacyModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnumLegacy::X(Option::Some(6543)),
    };

    // For a legacy model, keys and values must be serialized with Serde.

    let serialized_keys = dojo::model::model::ModelParser::<
        LegacyModelWithEnumKey,
    >::serialize_keys(@m);
    let serialized_values = dojo::model::model::ModelParser::<
        LegacyModelWithEnumKey,
    >::serialize_values(@m);

    assert_eq!(serialized_keys, [42, 2].span(), "LegacyModelWithEnumKey: serialize_keys failed");
    assert_eq!(
        serialized_values,
        [1234, 0, 5432, 0, 0, 6543].span(),
        "LegacyModelWithEnumKey: serialize_values failed",
    );

    let mut keys = [42, 2].span();
    let mut values = [1234, 0, 5432, 0, 0, 6543].span();

    assert_eq!(
        dojo::model::model::ModelParser::<
            LegacyModelWithEnumKey,
        >::deserialize(ref keys, ref values),
        Option::Some(m),
        "LegacyModelWithEnumKey: deserialize failed",
    );
}

#[test]
fn test_legacy_model_with_enum_key_single_model() {
    let mut world = spawn_foo_world();

    let m = LegacyModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnumLegacy::X(Option::Some(6543)),
    };

    // read uninitialized model, write and read back
    let def_m: LegacyModelWithEnumKey = world.read_model(m.keys());

    // For a legacy model, the default value is Some with variant data set to 0 for an option,
    // and the first variant with variant data set to 0 for an enum.
    assert!(
        def_m == LegacyModelWithEnumKey {
            k1: 42,
            k2: EnumKey::KEY_3,
            v1: 0,
            v2: Option::Some(0),
            v3: MyEnumLegacy::X(Option::Some(0)),
        },
        "LegacyModelWithEnumKey: read uninitialized model failed",
    );

    world.write_model(@m);

    let read_m: LegacyModelWithEnumKey = world.read_model(m.keys());
    assert!(m == read_m, "LegacyModelWithEnumKey: read model failed");
}

#[test]
fn test_legacy_model_with_enum_key_multiple_models() {
    let mut world = spawn_foo_world();

    let m1 = LegacyModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnumLegacy::X(Some(6543)),
    };
    let m2 = LegacyModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnumLegacy::Y((3, 4)),
    };
    let m3 = LegacyModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnumLegacy::Z,
    };

    // read uninitialized models
    let keys = [(11, EnumKey::KEY_1), (12, EnumKey::KEY_2), (13, EnumKey::KEY_3)].span();
    let models: Array<LegacyModelWithEnumKey> = world.read_models(keys);

    assert!(models.len() == 3, "LegacyModelWithEnumKey: read uninitialized models bad length");

    for (model, key) in models.into_iter().zip(keys) {
        let (k1, k2) = *key;
        assert!(
            model == LegacyModelWithEnumKey {
                k1, k2, v1: 0, v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)),
            },
        );
    }

    // write and read back them
    world.write_models([@m1, @m2, @m3].span());

    let read_models: Array<LegacyModelWithEnumKey> = world.read_models(keys);

    assert!(read_models.len() == 3, "LegacyModelWithEnumKey: read back models bad length");

    for (model, read_model) in [m1, m2, m3].span().into_iter().zip(read_models) {
        assert!(*model == read_model, "LegacyModelWithEnumKey: read back models bad content");
    }
}


#[test]
fn test_legacy_model_with_enum_key_member() {
    let mut world = spawn_foo_world();

    let m = LegacyModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnumLegacy::X(Option::Some(6543)),
    };

    // read uninitialized member
    let v2 = world.read_member_legacy(m.ptr(), selector!("v2"));
    let v3 = world.read_member_legacy(m.ptr(), selector!("v3"));
    assert!(v2 == Option::Some(0), "LegacyModelWithEnumKey: read uninitialized member v2 failed");
    assert!(
        v3 == MyEnumLegacy::X(Option::Some(0)),
        "LegacyModelWithEnumKey: read uninitialized member v3 failed",
    );

    // write and read back them
    world.write_member_legacy(m.ptr(), selector!("v2"), m.v2);
    world.write_member_legacy(m.ptr(), selector!("v3"), m.v3);

    let read_v2 = world.read_member_legacy(m.ptr(), selector!("v2"));
    let read_v3 = world.read_member_legacy(m.ptr(), selector!("v3"));

    assert!(read_v2 == m.v2, "LegacyModelWithEnumKey: read back member v2 failed");
    assert!(read_v3 == m.v3, "LegacyModelWithEnumKey: read back member v3 failed");
}


#[test]
fn test_legacy_model_with_enum_key_member_for_multiple_models() {
    let mut world = spawn_foo_world();

    let m1 = LegacyModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnumLegacy::X(Some(6543)),
    };
    let m2 = LegacyModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnumLegacy::Y((3, 4)),
    };
    let m3 = LegacyModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnumLegacy::Z,
    };

    let model_ptrs = [m1.ptr(), m2.ptr(), m3.ptr()].span();

    // read uninitialized member
    let v2s = world.read_member_of_models_legacy(model_ptrs, selector!("v2"));
    let v3s = world.read_member_of_models_legacy(model_ptrs, selector!("v3"));

    assert!(
        v2s.len() == 3, "LegacyModelWithEnumKey: read uninitialized member v2 of models bad length",
    );
    assert!(
        v3s.len() == 3, "LegacyModelWithEnumKey: read uninitialized member v3 of models bad length",
    );

    assert!(
        v2s == array![Option::Some(0), Option::Some(0), Option::Some(0)],
        "LegacyModelWithEnumKey: read uninitialized member v2 failed",
    );
    assert!(
        v3s == array![
            MyEnumLegacy::X(Option::Some(0)), MyEnumLegacy::X(Option::Some(0)),
            MyEnumLegacy::X(Option::Some(0)),
        ],
        "LegacyModelWithEnumKey: read uninitialized member v3 failed",
    );

    // write and read back them
    world
        .write_member_of_models_legacy(
            model_ptrs, selector!("v2"), array![m1.v2, m2.v2, m3.v2].span(),
        );
    world
        .write_member_of_models_legacy(
            model_ptrs, selector!("v3"), array![m1.v3, m2.v3, m3.v3].span(),
        );

    let read_v2s = world.read_member_of_models_legacy(model_ptrs, selector!("v2"));
    let read_v3s = world.read_member_of_models_legacy(model_ptrs, selector!("v3"));

    assert!(
        read_v2s.len() == 3, "LegacyModelWithEnumKey: read back member v2 of models bad length",
    );
    assert!(
        read_v3s.len() == 3, "LegacyModelWithEnumKey: read back member v3 of models bad length",
    );

    assert!(
        read_v2s == array![m1.v2, m2.v2, m3.v2],
        "LegacyModelWithEnumKey: read back member v2 of models failed",
    );
    assert!(
        read_v3s == array![m1.v3, m2.v3, m3.v3],
        "LegacyModelWithEnumKey: read back member v3 of models failed",
    );
}

#[test]
fn test_legacy_model_with_enum_key_schema() {
    let mut world = spawn_foo_world();

    let m = LegacyModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnumLegacy::X(Option::Some(6543)),
    };

    // read uninitialized schema
    let schema: LegacyModelSubset = world.read_schema_legacy(m.ptr());

    assert!(
        schema == LegacyModelSubset { v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)) },
        "LegacyModelWithEnumKey: read uninitialized schema failed",
    );

    // write and read back them
    world.write_model(@m);

    let read_schema: LegacyModelSubset = world.read_schema_legacy(m.ptr());

    assert!(
        read_schema == LegacyModelSubset { v2: m.v2, v3: m.v3 },
        "LegacyModelWithEnumKey: read back schema failed",
    );
}


#[test]
fn test_legacy_model_with_enum_key_schemas() {
    let mut world = spawn_foo_world();

    let m1 = LegacyModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnumLegacy::X(Some(6543)),
    };
    let m2 = LegacyModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnumLegacy::Y((3, 4)),
    };
    let m3 = LegacyModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnumLegacy::Z,
    };

    let model_ptrs = [m1.ptr(), m2.ptr(), m3.ptr()].span();

    // read uninitialized schema
    let schemas: Array<LegacyModelSubset> = world.read_schemas_legacy(model_ptrs);

    assert!(schemas.len() == 3, "LegacyModelWithEnumKey: read uninitialized schemas bad length");
    assert!(
        schemas == array![
            LegacyModelSubset { v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)) },
            LegacyModelSubset { v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)) },
            LegacyModelSubset { v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)) },
        ],
        "LegacyModelWithEnumKey: read uninitialized schemas failed",
    );

    // write and read back them
    world.write_models([@m1, @m2, @m3].span());

    let read_schemas: Array<LegacyModelSubset> = world.read_schemas_legacy(model_ptrs);

    assert!(
        read_schemas == array![
            LegacyModelSubset { v2: m1.v2, v3: m1.v3 }, LegacyModelSubset { v2: m2.v2, v3: m2.v3 },
            LegacyModelSubset { v2: m3.v2, v3: m3.v3 },
        ],
        "LegacyModelWithEnumKey: read back schemas failed",
    );
}

#[test]
fn test_legacy_model_with_enum_key_value() {
    let mut world = spawn_foo_world();

    let m = LegacyModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnumLegacy::X(Option::Some(6543)),
    };

    let mvalue = LegacyModelWithEnumKeyValue { v1: m.v1, v2: m.v2, v3: m.v3 };

    // read uninitialized model value, write and read back
    let def_mv: LegacyModelWithEnumKeyValue = world.read_value(m.keys());

    assert!(
        def_mv == LegacyModelWithEnumKeyValue {
            v1: 0, v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)),
        },
        "LegacyModelWithEnumKey: read uninitialized model value failed",
    );

    world.write_value(m.keys(), @mvalue);

    let read_mv: LegacyModelWithEnumKeyValue = world.read_value(m.keys());
    assert!(mvalue == read_mv, "LegacyModelWithEnumKey: read model value failed");
}

#[test]
fn test_legacy_model_with_enum_key_value_from_id() {
    let mut world = spawn_foo_world();

    let m = LegacyModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnumLegacy::X(Option::Some(6543)),
    };

    let mvalue = LegacyModelWithEnumKeyValue { v1: m.v1, v2: m.v2, v3: m.v3 };

    // read uninitialized model value, write and read back
    let def_mv: LegacyModelWithEnumKeyValue = world
        .read_value_from_id(dojo::utils::entity_id_from_keys(@m.keys()));

    assert!(
        def_mv == LegacyModelWithEnumKeyValue {
            v1: 0, v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)),
        },
        "LegacyModelWithEnumKey: read uninitialized model value from id failed",
    );

    world.write_value_from_id(dojo::utils::entity_id_from_keys(@m.keys()), @mvalue);

    let read_mv: LegacyModelWithEnumKeyValue = world
        .read_value_from_id(dojo::utils::entity_id_from_keys(@m.keys()));
    assert!(mvalue == read_mv, "LegacyModelWithEnumKey: read model value from id failed");
}

#[test]
fn test_legacy_model_with_enum_key_values() {
    let mut world = spawn_foo_world();

    let m1 = LegacyModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnumLegacy::X(Some(6543)),
    };
    let m2 = LegacyModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnumLegacy::Y((3, 4)),
    };
    let m3 = LegacyModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnumLegacy::Z,
    };

    let m1value = LegacyModelWithEnumKeyValue { v1: m1.v1, v2: m1.v2, v3: m1.v3 };
    let m2value = LegacyModelWithEnumKeyValue { v1: m2.v1, v2: m2.v2, v3: m2.v3 };
    let m3value = LegacyModelWithEnumKeyValue { v1: m3.v1, v2: m3.v2, v3: m3.v3 };

    let keys = [m1.keys(), m2.keys(), m3.keys()].span();

    // read uninitialized model value, write and read back
    let def_mvs: Array<LegacyModelWithEnumKeyValue> = world.read_values(keys);

    assert!(
        def_mvs == array![
            LegacyModelWithEnumKeyValue {
                v1: 0, v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)),
            },
            LegacyModelWithEnumKeyValue {
                v1: 0, v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)),
            },
            LegacyModelWithEnumKeyValue {
                v1: 0, v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)),
            },
        ],
        "LegacyModelWithEnumKey: read uninitialized model values failed",
    );

    world.write_values(keys, [@m1value, @m2value, @m3value].span());

    let read_mvs: Array<LegacyModelWithEnumKeyValue> = world.read_values(keys);
    assert!(
        read_mvs == array![m1value, m2value, m3value],
        "LegacyModelWithEnumKey: read model values failed",
    );
}

#[test]
fn test_legacy_model_with_enum_key_values_from_ids() {
    let mut world = spawn_foo_world();

    let m1 = LegacyModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnumLegacy::X(Some(6543)),
    };
    let m2 = LegacyModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnumLegacy::Y((3, 4)),
    };
    let m3 = LegacyModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnumLegacy::Z,
    };

    let m1value = LegacyModelWithEnumKeyValue { v1: m1.v1, v2: m1.v2, v3: m1.v3 };
    let m2value = LegacyModelWithEnumKeyValue { v1: m2.v1, v2: m2.v2, v3: m2.v3 };
    let m3value = LegacyModelWithEnumKeyValue { v1: m3.v1, v2: m3.v2, v3: m3.v3 };

    let ids = [
        dojo::utils::entity_id_from_keys(@m1.keys()), dojo::utils::entity_id_from_keys(@m2.keys()),
        dojo::utils::entity_id_from_keys(@m3.keys()),
    ]
        .span();

    // read uninitialized model value, write and read back
    let def_mvs: Array<LegacyModelWithEnumKeyValue> = world.read_values_from_ids(ids);

    assert!(
        def_mvs == array![
            LegacyModelWithEnumKeyValue {
                v1: 0, v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)),
            },
            LegacyModelWithEnumKeyValue {
                v1: 0, v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)),
            },
            LegacyModelWithEnumKeyValue {
                v1: 0, v2: Option::Some(0), v3: MyEnumLegacy::X(Option::Some(0)),
            },
        ],
        "LegacyModelWithEnumKey: read uninitialized model values from ids failed",
    );

    world.write_values_from_ids(ids, [@m1value, @m2value, @m3value].span());

    let read_mvs: Array<LegacyModelWithEnumKeyValue> = world.read_values_from_ids(ids);
    assert!(
        read_mvs == array![m1value, m2value, m3value],
        "LegacyModelWithEnumKey: read model values from ids failed",
    );
}

#[test]
fn test_legacy_model_with_enum_key_test_helpers() {
    let mut world = spawn_foo_world();

    let mut m1 = LegacyModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnumLegacy::X(Some(6543)),
    };
    let mut m2 = LegacyModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnumLegacy::Y((3, 4)),
    };
    let mut m3 = LegacyModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnumLegacy::Z,
    };

    let m1value = LegacyModelWithEnumKeyValue { v1: m1.v1, v2: m1.v2, v3: m1.v3 };
    let m2value = LegacyModelWithEnumKeyValue { v1: m2.v1, v2: m2.v2, v3: m2.v3 };
    let m3value = LegacyModelWithEnumKeyValue { v1: m3.v1, v2: m3.v2, v3: m3.v3 };

    /// --- write_model_test() ---

    world.write_model_test(@m1);

    let read_m1: LegacyModelWithEnumKey = world.read_model(m1.keys());
    assert!(m1 == read_m1, "LegacyModelWithEnumKey: write model test failed");

    /// --- write_models_test() ---

    m1.k1 = 21;

    world.write_models_test([@m1, @m2, @m3].span());

    let read_m: Array<LegacyModelWithEnumKey> = world
        .read_models([m1.keys(), m2.keys(), m3.keys()].span());
    assert!(read_m == array![m1, m2, m3], "LegacyModelWithEnumKey: write models test failed");

    /// --- write_value_test() ---

    m1.k1 = 31;

    world.write_value_test(m1.keys(), @m1value);

    let read_m1value: LegacyModelWithEnumKeyValue = world.read_value(m1.keys());
    assert!(m1value == read_m1value, "LegacyModelWithEnumKey: write value test failed");

    //  --- write_values_test() ---

    m1.k1 = 41;
    m2.k1 = 42;
    m3.k1 = 43;

    world
        .write_values_test(
            [m1.keys(), m2.keys(), m3.keys()].span(), [@m1value, @m2value, @m3value].span(),
        );

    let read_mvs: Array<LegacyModelWithEnumKeyValue> = world
        .read_values([m1.keys(), m2.keys(), m3.keys()].span());
    assert!(
        read_mvs == array![m1value, m2value, m3value],
        "LegacyModelWithEnumKey: write values test failed",
    );

    // --- write_value_from_id_test() ---

    m1.k1 = 51;

    world.write_value_from_id_test(dojo::utils::entity_id_from_keys(@m1.keys()), @m1value);

    let read_m1value: LegacyModelWithEnumKeyValue = world.read_value(m1.keys());
    assert!(m1value == read_m1value, "LegacyModelWithEnumKey: write value from id test failed");

    // --- write_values_from_ids_test() ---

    m1.k1 = 61;
    m2.k1 = 62;
    m3.k1 = 63;

    world
        .write_values_from_ids_test(
            [
                dojo::utils::entity_id_from_keys(@m1.keys()),
                dojo::utils::entity_id_from_keys(@m2.keys()),
                dojo::utils::entity_id_from_keys(@m3.keys()),
            ]
                .span(),
            [@m1value, @m2value, @m3value].span(),
        );

    let read_mvs: Array<LegacyModelWithEnumKeyValue> = world
        .read_values([m1.keys(), m2.keys(), m3.keys()].span());
    assert!(
        read_mvs == array![m1value, m2value, m3value],
        "LegacyModelWithEnumKey: write values from ids test failed",
    );
}

#[test]
fn test_dojo_store_model_with_enum_key_serialization() {
    let m = DojoStoreModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnum::X(Option::Some(6543)),
    };

    // For a DojoStore model, keys must be serialized with Serde while values must be serialized
    // with DojoStore.

    let serialized_keys = dojo::model::model::ModelParser::<
        DojoStoreModelWithEnumKey,
    >::serialize_keys(@m);
    let serialized_values = dojo::model::model::ModelParser::<
        DojoStoreModelWithEnumKey,
    >::serialize_values(@m);

    assert_eq!(serialized_keys, [42, 2].span(), "DojoStoreModelWithEnumKey: serialize_keys failed");
    assert_eq!(
        serialized_values,
        [1234, 1, 5432, 1, 1, 6543].span(),
        "DojoStoreModelWithEnumKey: serialize_values failed",
    );

    let mut keys = [42, 2].span();
    let mut values = [1234, 1, 5432, 1, 1, 6543].span();

    assert_eq!(
        dojo::model::model::ModelParser::<
            DojoStoreModelWithEnumKey,
        >::deserialize(ref keys, ref values),
        Option::Some(m),
        "DojoStoreModelWithEnumKey: deserialize failed",
    );
}

#[test]
fn test_dojo_store_model_with_enum_key_single_model() {
    let mut world = spawn_foo_world();

    let m = DojoStoreModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnum::X(Option::Some(6543)),
    };

    // read uninitialized model, write and read back
    let def_m: DojoStoreModelWithEnumKey = world.read_model(m.keys());

    // for a DojoStore model, the default value is None for options and the configured default value
    // for enums.
    assert!(
        def_m == DojoStoreModelWithEnumKey {
            k1: 42, k2: EnumKey::KEY_3, v1: 0, v2: Option::None, v3: MyEnum::Z,
        },
        "DojoStoreModelWithEnumKey: read uninitialized model failed",
    );

    world.write_model(@m);

    let read_m: DojoStoreModelWithEnumKey = world.read_model(m.keys());
    assert!(m == read_m, "DojoStoreModelWithEnumKey: read model failed");
}


#[test]
fn test_dojo_store_model_with_enum_key_multiple_models() {
    let mut world = spawn_foo_world();

    let m1 = DojoStoreModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnum::X(Some(6543)),
    };
    let m2 = DojoStoreModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnum::Y((3, 4)),
    };
    let m3 = DojoStoreModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnum::Z,
    };

    // read uninitialized models
    let keys = [(11, EnumKey::KEY_1), (12, EnumKey::KEY_2), (13, EnumKey::KEY_3)].span();
    let models: Array<DojoStoreModelWithEnumKey> = world.read_models(keys);

    assert!(models.len() == 3, "DojoStoreModelWithEnumKey: read uninitialized models bad length");

    for (model, key) in models.into_iter().zip(keys) {
        let (k1, k2) = *key;
        assert!(model == DojoStoreModelWithEnumKey { k1, k2, v1: 0, v2: None, v3: MyEnum::Z });
    }

    // write and read back them
    world.write_models([@m1, @m2, @m3].span());

    let read_models: Array<DojoStoreModelWithEnumKey> = world.read_models(keys);

    assert!(read_models.len() == 3, "DojoStoreModelWithEnumKey: read back models bad length");

    for (model, read_model) in [m1, m2, m3].span().into_iter().zip(read_models) {
        assert!(*model == read_model, "DojoStoreModelWithEnumKey: read back models bad content");
    }
}


#[test]
fn test_dojo_store_model_with_enum_key_schema() {
    let mut world = spawn_foo_world();

    let m = DojoStoreModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnum::X(Option::Some(6543)),
    };

    // read uninitialized schema
    let schema: DojoStoreModelSubset = world.read_schema(m.ptr());

    assert!(
        schema == DojoStoreModelSubset { v2: Option::None, v3: MyEnum::Z },
        "DojoStoreModelWithEnumKey: read uninitialized schema failed",
    );

    // write and read back them
    world.write_model(@m);

    let read_schema: DojoStoreModelSubset = world.read_schema(m.ptr());

    assert!(
        read_schema == DojoStoreModelSubset { v2: m.v2, v3: m.v3 },
        "DojoStoreModelWithEnumKey: read back schema failed",
    );
}

#[test]
fn test_dojo_store_model_with_enum_key_schemas() {
    let mut world = spawn_foo_world();

    let m1 = DojoStoreModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnum::X(Some(6543)),
    };
    let m2 = DojoStoreModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnum::Y((3, 4)),
    };
    let m3 = DojoStoreModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnum::Z,
    };

    let model_ptrs = [m1.ptr(), m2.ptr(), m3.ptr()].span();

    // read uninitialized schema
    let schemas: Array<DojoStoreModelSubset> = world.read_schemas(model_ptrs);

    assert!(schemas.len() == 3, "DojoStoreModelWithEnumKey: read uninitialized schemas bad length");
    assert!(
        schemas == array![
            DojoStoreModelSubset { v2: Option::None, v3: MyEnum::Z },
            DojoStoreModelSubset { v2: Option::None, v3: MyEnum::Z },
            DojoStoreModelSubset { v2: Option::None, v3: MyEnum::Z },
        ],
        "DojoStoreModelWithEnumKey: read uninitialized schemas failed",
    );

    // write and read back them
    world.write_models([@m1, @m2, @m3].span());

    let read_schemas: Array<DojoStoreModelSubset> = world.read_schemas(model_ptrs);

    assert!(
        read_schemas == array![
            DojoStoreModelSubset { v2: m1.v2, v3: m1.v3 },
            DojoStoreModelSubset { v2: m2.v2, v3: m2.v3 },
            DojoStoreModelSubset { v2: m3.v2, v3: m3.v3 },
        ],
        "DojoStoreModelWithEnumKey: read back schemas failed",
    );
}

#[test]
fn test_dojo_store_model_with_enum_key_member() {
    let mut world = spawn_foo_world();

    let m = DojoStoreModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnum::X(Option::Some(6543)),
    };

    // read uninitialized member
    let v2: Option<u32> = world.read_member(m.ptr(), selector!("v2"));
    let v3: MyEnum = world.read_member(m.ptr(), selector!("v3"));
    assert!(v2 == None, "DojoStoreModelWithEnumKey: read uninitialized member v2 failed");
    assert!(v3 == MyEnum::Z, "DojoStoreModelWithEnumKey: read uninitialized member v3 failed");

    // write and read back them
    world.write_member(m.ptr(), selector!("v2"), m.v2);
    world.write_member(m.ptr(), selector!("v3"), m.v3);

    let read_v2: Option<u32> = world.read_member(m.ptr(), selector!("v2"));
    let read_v3: MyEnum = world.read_member(m.ptr(), selector!("v3"));

    assert!(read_v2 == m.v2, "DojoStoreModelWithEnumKey: read back member v2 failed");
    assert!(read_v3 == m.v3, "DojoStoreModelWithEnumKey: read back member v3 failed");
}

#[test]
fn test_dojo_store_model_with_enum_key_member_for_multiple_models() {
    let mut world = spawn_foo_world();

    let m1 = DojoStoreModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnum::X(Some(6543)),
    };
    let m2 = DojoStoreModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnum::Y((3, 4)),
    };
    let m3 = DojoStoreModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnum::Z,
    };

    let model_ptrs = [m1.ptr(), m2.ptr(), m3.ptr()].span();

    // read uninitialized member
    let v2s: Array<Option<u32>> = world.read_member_of_models(model_ptrs, selector!("v2"));
    let v3s: Array<MyEnum> = world.read_member_of_models(model_ptrs, selector!("v3"));

    assert!(
        v2s.len() == 3,
        "DojoStoreModelWithEnumKey: read uninitialized member v2 of models bad length",
    );
    assert!(
        v3s.len() == 3,
        "DojoStoreModelWithEnumKey: read uninitialized member v3 of models bad length",
    );

    assert!(
        v2s == array![Option::None, Option::None, Option::None],
        "DojoStoreModelWithEnumKey: read uninitialized member v2 failed",
    );
    assert!(
        v3s == array![MyEnum::Z, MyEnum::Z, MyEnum::Z],
        "DojoStoreModelWithEnumKey: read uninitialized member v3 failed",
    );

    // write and read back them
    world.write_member_of_models(model_ptrs, selector!("v2"), array![m1.v2, m2.v2, m3.v2].span());
    world.write_member_of_models(model_ptrs, selector!("v3"), array![m1.v3, m2.v3, m3.v3].span());

    let read_v2s = world.read_member_of_models(model_ptrs, selector!("v2"));
    let read_v3s = world.read_member_of_models(model_ptrs, selector!("v3"));

    assert!(
        read_v2s.len() == 3, "DojoStoreModelWithEnumKey: read back member v2 of models bad length",
    );
    assert!(
        read_v3s.len() == 3, "DojoStoreModelWithEnumKey: read back member v3 of models bad length",
    );

    assert!(
        read_v2s == array![m1.v2, m2.v2, m3.v2],
        "DojoStoreModelWithEnumKey: read back member v2 of models failed",
    );
    assert!(
        read_v3s == array![m1.v3, m2.v3, m3.v3],
        "DojoStoreModelWithEnumKey: read back member v3 of models failed",
    );
}


#[test]
fn test_dojo_store_model_with_enum_key_value() {
    let mut world = spawn_foo_world();

    let m = DojoStoreModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnum::X(Option::Some(6543)),
    };

    let mvalue = DojoStoreModelWithEnumKeyValue { v1: m.v1, v2: m.v2, v3: m.v3 };

    // read uninitialized model value, write and read back
    let def_mv: DojoStoreModelWithEnumKeyValue = world.read_value(m.keys());

    assert!(
        def_mv == DojoStoreModelWithEnumKeyValue { v1: 0, v2: Option::None, v3: MyEnum::Z },
        "DojoStoreModelWithEnumKey: read uninitialized model value failed",
    );

    world.write_value(m.keys(), @mvalue);

    let read_mv: DojoStoreModelWithEnumKeyValue = world.read_value(m.keys());
    assert!(mvalue == read_mv, "DojoStoreModelWithEnumKey: read model value failed");
}


#[test]
fn test_dojo_store_model_with_enum_key_value_from_id() {
    let mut world = spawn_foo_world();

    let m = DojoStoreModelWithEnumKey {
        k1: 42,
        k2: EnumKey::KEY_3,
        v1: 1234,
        v2: Option::Some(5432),
        v3: MyEnum::X(Option::Some(6543)),
    };

    let mvalue = DojoStoreModelWithEnumKeyValue { v1: m.v1, v2: m.v2, v3: m.v3 };

    // read uninitialized model value, write and read back
    let def_mv: DojoStoreModelWithEnumKeyValue = world
        .read_value_from_id(dojo::utils::entity_id_from_keys(@m.keys()));

    assert!(
        def_mv == DojoStoreModelWithEnumKeyValue { v1: 0, v2: Option::None, v3: MyEnum::Z },
        "DojoStoreModelWithEnumKey: read uninitialized model value from id failed",
    );

    world.write_value_from_id(dojo::utils::entity_id_from_keys(@m.keys()), @mvalue);

    let read_mv: DojoStoreModelWithEnumKeyValue = world
        .read_value_from_id(dojo::utils::entity_id_from_keys(@m.keys()));
    assert!(mvalue == read_mv, "DojoStoreModelWithEnumKey: read model value from id failed");
}


#[test]
fn test_dojo_store_model_with_enum_key_values() {
    let mut world = spawn_foo_world();

    let m1 = DojoStoreModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnum::X(Some(6543)),
    };
    let m2 = DojoStoreModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnum::Y((3, 4)),
    };
    let m3 = DojoStoreModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnum::Z,
    };

    let m1value = DojoStoreModelWithEnumKeyValue { v1: m1.v1, v2: m1.v2, v3: m1.v3 };
    let m2value = DojoStoreModelWithEnumKeyValue { v1: m2.v1, v2: m2.v2, v3: m2.v3 };
    let m3value = DojoStoreModelWithEnumKeyValue { v1: m3.v1, v2: m3.v2, v3: m3.v3 };

    let keys = [m1.keys(), m2.keys(), m3.keys()].span();

    // read uninitialized model value, write and read back
    let def_mvs: Array<DojoStoreModelWithEnumKeyValue> = world.read_values(keys);

    assert!(
        def_mvs == array![
            DojoStoreModelWithEnumKeyValue { v1: 0, v2: Option::None, v3: MyEnum::Z },
            DojoStoreModelWithEnumKeyValue { v1: 0, v2: Option::None, v3: MyEnum::Z },
            DojoStoreModelWithEnumKeyValue { v1: 0, v2: Option::None, v3: MyEnum::Z },
        ],
        "DojoStoreModelWithEnumKey: read uninitialized model values failed",
    );

    world.write_values(keys, [@m1value, @m2value, @m3value].span());

    let read_mvs: Array<DojoStoreModelWithEnumKeyValue> = world.read_values(keys);
    assert!(
        read_mvs == array![m1value, m2value, m3value],
        "DojoStoreModelWithEnumKey: read model values failed",
    );
}


#[test]
fn test_dojo_store_model_with_enum_key_values_from_ids() {
    let mut world = spawn_foo_world();

    let m1 = DojoStoreModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnum::X(Some(6543)),
    };
    let m2 = DojoStoreModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnum::Y((3, 4)),
    };
    let m3 = DojoStoreModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnum::Z,
    };

    let m1value = DojoStoreModelWithEnumKeyValue { v1: m1.v1, v2: m1.v2, v3: m1.v3 };
    let m2value = DojoStoreModelWithEnumKeyValue { v1: m2.v1, v2: m2.v2, v3: m2.v3 };
    let m3value = DojoStoreModelWithEnumKeyValue { v1: m3.v1, v2: m3.v2, v3: m3.v3 };

    let ids = [
        dojo::utils::entity_id_from_keys(@m1.keys()), dojo::utils::entity_id_from_keys(@m2.keys()),
        dojo::utils::entity_id_from_keys(@m3.keys()),
    ]
        .span();

    // read uninitialized model value, write and read back
    let def_mvs: Array<DojoStoreModelWithEnumKeyValue> = world.read_values_from_ids(ids);

    assert!(
        def_mvs == array![
            DojoStoreModelWithEnumKeyValue { v1: 0, v2: Option::None, v3: MyEnum::Z },
            DojoStoreModelWithEnumKeyValue { v1: 0, v2: Option::None, v3: MyEnum::Z },
            DojoStoreModelWithEnumKeyValue { v1: 0, v2: Option::None, v3: MyEnum::Z },
        ],
        "DojoStoreModelWithEnumKey: read uninitialized model values from ids failed",
    );

    world.write_values_from_ids(ids, [@m1value, @m2value, @m3value].span());

    let read_mvs: Array<DojoStoreModelWithEnumKeyValue> = world.read_values_from_ids(ids);
    assert!(
        read_mvs == array![m1value, m2value, m3value],
        "DojoStoreModelWithEnumKey: read model values from ids failed",
    );
}


#[test]
fn test_dojo_store_model_with_enum_key_test_helpers() {
    let mut world = spawn_foo_world();

    let mut m1 = DojoStoreModelWithEnumKey {
        k1: 11, k2: EnumKey::KEY_1, v1: 1234, v2: Some(5432), v3: MyEnum::X(Some(6543)),
    };
    let mut m2 = DojoStoreModelWithEnumKey {
        k1: 12, k2: EnumKey::KEY_2, v1: 4567, v2: None, v3: MyEnum::Y((3, 4)),
    };
    let mut m3 = DojoStoreModelWithEnumKey {
        k1: 13, k2: EnumKey::KEY_3, v1: 1234, v2: Option::Some(9999), v3: MyEnum::Z,
    };

    let m1value = DojoStoreModelWithEnumKeyValue { v1: m1.v1, v2: m1.v2, v3: m1.v3 };
    let m2value = DojoStoreModelWithEnumKeyValue { v1: m2.v1, v2: m2.v2, v3: m2.v3 };
    let m3value = DojoStoreModelWithEnumKeyValue { v1: m3.v1, v2: m3.v2, v3: m3.v3 };

    /// --- write_model_test() ---

    world.write_model_test(@m1);

    let read_m1: DojoStoreModelWithEnumKey = world.read_model(m1.keys());
    assert!(m1 == read_m1, "DojoStoreModelWithEnumKey: write model test failed");

    /// --- write_models_test() ---

    m1.k1 = 21;

    world.write_models_test([@m1, @m2, @m3].span());

    let read_m: Array<DojoStoreModelWithEnumKey> = world
        .read_models([m1.keys(), m2.keys(), m3.keys()].span());
    assert!(read_m == array![m1, m2, m3], "DojoStoreModelWithEnumKey: write models test failed");

    /// --- write_value_test() ---

    m1.k1 = 31;

    world.write_value_test(m1.keys(), @m1value);

    let read_m1value: DojoStoreModelWithEnumKeyValue = world.read_value(m1.keys());
    assert!(m1value == read_m1value, "DojoStoreModelWithEnumKey: write value test failed");

    //  --- write_values_test() ---

    m1.k1 = 41;
    m2.k1 = 42;
    m3.k1 = 43;

    world
        .write_values_test(
            [m1.keys(), m2.keys(), m3.keys()].span(), [@m1value, @m2value, @m3value].span(),
        );

    let read_mvs: Array<DojoStoreModelWithEnumKeyValue> = world
        .read_values([m1.keys(), m2.keys(), m3.keys()].span());
    assert!(
        read_mvs == array![m1value, m2value, m3value],
        "DojoStoreModelWithEnumKey: write values test failed",
    );

    // --- write_value_from_id_test() ---

    m1.k1 = 51;

    world.write_value_from_id_test(dojo::utils::entity_id_from_keys(@m1.keys()), @m1value);

    let read_m1value: DojoStoreModelWithEnumKeyValue = world.read_value(m1.keys());
    assert!(m1value == read_m1value, "DojoStoreModelWithEnumKey: write value from id test failed");

    // --- write_values_from_ids_test() ---

    m1.k1 = 61;
    m2.k1 = 62;
    m3.k1 = 63;

    world
        .write_values_from_ids_test(
            [
                dojo::utils::entity_id_from_keys(@m1.keys()),
                dojo::utils::entity_id_from_keys(@m2.keys()),
                dojo::utils::entity_id_from_keys(@m3.keys()),
            ]
                .span(),
            [@m1value, @m2value, @m3value].span(),
        );

    let read_mvs: Array<DojoStoreModelWithEnumKeyValue> = world
        .read_values([m1.keys(), m2.keys(), m3.keys()].span());
    assert!(
        read_mvs == array![m1value, m2value, m3value],
        "DojoStoreModelWithEnumKey: write values from ids test failed",
    );
}
