use dojo::test_utils::{spawn_test_world};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

// Utils
fn deploy_world() -> IWorldDispatcher {
    spawn_test_world("dojo", array![])
}

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

#[test]
fn test_id() {
    let mvalues = FooEntity { __id: 1, v1: 3, v2: 4 };
    assert!(mvalues.id() == 1);
}

#[test]
fn test_values() {
    let mvalues = FooEntity { __id: 1, v1: 3, v2: 4 };
    let expected_values = array![3, 4].span();

    let values = dojo::model::ModelEntity::<FooEntity>::values(@mvalues);
    assert!(expected_values == values);
}

#[test]
fn test_from_values() {
    let values = array![3, 4].span();

    let model_entity = dojo::model::ModelEntity::<FooEntity>::from_values(1, values);
    assert!(model_entity.__id == 1 && model_entity.v1 == 3 && model_entity.v2 == 4);
}

#[test]
#[should_panic(expected: "ModelEntity `FooEntity`: deserialization failed.")]
fn test_from_values_bad_data() {
    let values = array![3].span();
    let _ = dojo::model::ModelEntity::<FooEntity>::from_values(1, values);
}

#[test]
fn test_get_and_update_entity() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    foo.set(world);

    let entity_id = foo.entity_id();
    let mut entity = FooEntityTrait::get(world, entity_id);
    assert!(entity.__id == entity_id && entity.v1 == entity.v1 && entity.v2 == entity.v2);

    entity.v1 = 12;
    entity.v2 = 18;

    entity.update(world);

    let read_values = FooEntityTrait::get(world, entity_id);
    assert!(read_values.v1 == entity.v1 && read_values.v2 == entity.v2);
}

#[test]
fn test_delete_entity() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    foo.set(world);

    let entity_id = foo.entity_id();
    let mut entity = FooEntityTrait::get(world, entity_id);
    entity.delete(world);

    let read_values = FooEntityTrait::get(world, entity_id);
    assert!(read_values.v1 == 0 && read_values.v2 == 0);
}

#[test]
fn test_get_and_set_member_from_entity() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    foo.set(world);

    let v1_raw_value: Span<felt252> = dojo::model::ModelEntity::<
        FooEntity
    >::get_member(world, foo.entity_id(), selector!("v1"));

    assert!(v1_raw_value.len() == 1);
    assert!(*v1_raw_value.at(0) == 3);

    let entity = FooEntityTrait::get(world, foo.entity_id());
    entity.set_member(world, selector!("v1"), array![42].span());

    let entity = FooEntityTrait::get(world, foo.entity_id());
    assert!(entity.v1 == 42);
}

#[test]
fn test_get_and_set_field_name() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    foo.set(world);

    let v1 = FooEntityTrait::get_v1(world, foo.entity_id());
    assert!(foo.v1 == v1);

    let entity = FooEntityTrait::get(world, foo.entity_id());
    entity.set_v1(world, 42);

    let v1 = FooEntityTrait::get_v1(world, foo.entity_id());
    assert!(v1 == 42);
}

#[test]
fn test_get_and_set_from_model() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    foo.set(world);

    let read_entity = FooTrait::get(world, foo.k1, foo.k2);

    assert!(
        foo.k1 == read_entity.k1
            && foo.k2 == read_entity.k2
            && foo.v1 == read_entity.v1
            && foo.v2 == read_entity.v2
    );
}

#[test]
fn test_delete_from_model() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    foo.set(world);
    foo.delete(world);

    let read_entity = FooTrait::get(world, foo.k1, foo.k2);
    assert!(
        read_entity.k1 == foo.k1
            && read_entity.k2 == foo.k2
            && read_entity.v1 == 0
            && read_entity.v2 == 0
    );
}

#[test]
fn test_get_and_set_member_from_model() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    let keys = array![foo.k1.into(), foo.k2.into()].span();
    foo.set(world);

    let v1_raw_value = dojo::model::Model::<Foo>::get_member(world, keys, selector!("v1"));

    assert!(v1_raw_value.len() == 1);
    assert!(*v1_raw_value.at(0) == 3);

    foo.set_member(world, selector!("v1"), array![42].span());
    let foo = FooTrait::get(world, foo.k1, foo.k2);
    assert!(foo.v1 == 42);
}

#[test]
fn test_get_and_set_field_name_from_model() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    foo.set(world);

    let v1 = FooTrait::get_v1(world, foo.k1, foo.k2);
    assert!(v1 == 3);

    foo.set_v1(world, 42);

    let v1 = FooTrait::get_v1(world, foo.k1, foo.k2);
    assert!(v1 == 42);
}

