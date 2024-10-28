use dojo::model::{Model, ModelValue, ModelStorage, ModelValueStorage, ModelMemberStorage};
use dojo::world::{IWorldDispatcherTrait, WorldStorageTrait, WorldStorage};

use crate::tests::helpers::{deploy_world};

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::model]
struct Foo {
    #[key]
    k1: u8,
    #[key]
    k2: felt252,
    v1: u128,
    v2: u32
}


#[derive(Copy, Drop, Serde, Debug)]
#[dojo::model]
struct Foo2 {
    #[key]
    k1: u8,
    #[key]
    k2: felt252,
    v1: u128,
    v2: u32
}

#[test]
fn test_model_definition() {
    let definition = dojo::model::Model::<Foo>::definition();

    assert_eq!(definition.name, dojo::model::Model::<Foo>::name());
    assert_eq!(definition.version, dojo::model::Model::<Foo>::version());
    assert_eq!(definition.layout, dojo::model::Model::<Foo>::layout());
    assert_eq!(definition.schema, dojo::model::Model::<Foo>::schema());
    assert_eq!(definition.packed_size, dojo::model::Model::<Foo>::packed_size());
    assert_eq!(definition.unpacked_size, dojo::meta::introspect::Introspect::<Foo>::size());
}

#[test]
fn test_values() {
    let mvalues = FooValue { v1: 3, v2: 4 };
    let expected_values = [3, 4].span();

    let values = mvalues.values();
    assert!(expected_values == values);
}

#[test]
fn test_from_values() {
    let mut values = [3, 4].span();

    let model_values: Option<FooValue> = ModelValue::<FooValue>::from_values(1, ref values);
    assert!(model_values.is_some());
    let model_values = model_values.unwrap();
    assert!(model_values.v1 == 3 && model_values.v2 == 4);
}

#[test]
fn test_from_values_bad_data() {
    let mut values = [3].span();
    let res: Option<FooValue> = ModelValue::<FooValue>::from_values(1, ref values);
    assert!(res.is_none());
}

#[test]
fn test_get_and_update_model_value() {
    let world = deploy_world();
    world.register_model("dojo", foo::TEST_CLASS_HASH.try_into().unwrap());

    let mut world = WorldStorageTrait::new(world, "dojo");

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);

    let entity_id = foo.entity_id();
    let mut model_value: FooValue = world.read_model_value(foo.key());
    assert_eq!(model_value.v1, foo.v1);
    assert_eq!(model_value.v2, foo.v2);

    model_value.v1 = 12;
    model_value.v2 = 18;

    world.write_model_value_from_id(entity_id, @model_value);

    let read_values: FooValue = world.read_model_value(foo.key());
    assert!(read_values.v1 == model_value.v1 && read_values.v2 == model_value.v2);
}

#[test]
fn test_delete_model_value() {
    let world = deploy_world();
    world.register_model("dojo", foo::TEST_CLASS_HASH.try_into().unwrap());

    let mut world = WorldStorageTrait::new(world, "dojo");

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);

    let entity_id = foo.entity_id();
    ModelStorage::<WorldStorage, Foo>::erase_model(ref world, @foo);

    let read_values: FooValue = world.read_model_value_from_id(entity_id);
    assert!(read_values.v1 == 0 && read_values.v2 == 0);
}

#[test]
fn test_get_and_set_field_name() {
    let world = deploy_world();
    world.register_model("dojo", foo::TEST_CLASS_HASH.try_into().unwrap());

    let mut world = WorldStorageTrait::new(world, "dojo");

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);

    // Inference fails here, we need something better without too generics
    // which also fails.
    let v1 = world.read_member(foo.key(), selector!("v1"));
    assert!(foo.v1 == v1);

    world.write_member_from_id(foo.entity_id(), selector!("v1"), 42);

    let v1 = world.read_member_from_id(foo.key(), selector!("v1"));
    assert!(v1 == 42);
}

#[test]
fn test_get_and_set_from_model() {
    let world = deploy_world();
    world.register_model("dojo", foo::TEST_CLASS_HASH.try_into().unwrap());

    let mut world = WorldStorageTrait::new(world, "dojo");

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);

    let foo2: Foo = world.read_model((foo.k1, foo.k2));

    assert!(
        foo.k1 == foo2.k1
            && foo.k2 == foo2.k2
            && foo.v1 == foo2.v1
            && foo.v2 == foo2.v2
    );
}

#[test]
fn test_delete_from_model() {
    let world = deploy_world();
    world.register_model("dojo", foo::TEST_CLASS_HASH.try_into().unwrap());

    let mut world = WorldStorageTrait::new(world, "dojo");

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);
    world.erase_model(@foo);

    let foo2: Foo = world.read_model((foo.k1, foo.k2));
    assert!(
        foo2.k1 == foo.k1
            && foo2.k2 == foo.k2
            && foo2.v1 == 0
            && foo2.v2 == 0
    );
}

#[test]
fn test_get_and_set_member_from_model() {
    let world = deploy_world();
    world.register_model("dojo", foo::TEST_CLASS_HASH.try_into().unwrap());

    let mut world = WorldStorageTrait::new(world, "dojo");

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);

    let key: (u8, felt252) = foo.key();
    let v1: u128 = world.read_member(key, selector!("v1"));

    assert!(v1 == 3);

    world.write_member(key, selector!("v1"), 42);
    let foo: Foo = world.read_model(key);
    assert!(foo.v1 == 42);
}

#[test]
fn test_get_and_set_field_name_from_model() {
    let world = deploy_world();
    world.register_model("dojo", foo::TEST_CLASS_HASH.try_into().unwrap());

    let mut world = WorldStorageTrait::new(world, "dojo");

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.write_model(@foo);

    // Currently we don't have automatic field id computation. To be done.
    // @remy/@ben.

    let v1 = world.read_member((foo.k1, foo.k2), selector!("v1"));
    assert!(v1 == 3);

    world.write_member((foo.k1, foo.k2), selector!("v1"), 42);
    assert!(v1 == 42);
}
