use dojo::test_utils::{spawn_test_world};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

// Utils
fn deploy_world() -> IWorldDispatcher {
    spawn_test_world("dojo", array![])
}

#[derive(Copy, Drop, Serde)]
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
fn test_values() {
    let mvalues = FooValues { v1: 3, v2: 4 };
    let expected_values = array![3, 4].span();

    let values = FooModelValues::values(@mvalues);
    assert!(expected_values == values);
}

#[test]
fn test_from_values() {
    let values = array![3, 4].span();

    let model_values = FooModelValues::from_values(values);
    assert!(model_values.v1 == 3 && model_values.v2 == 4);
}

#[test]
#[should_panic(expected: "ModelValues `FooValues`: deserialization failed.")]
fn test_from_values_bad_data() {
    let values = array![3].span();
    let _ = FooModelValues::from_values(values);
}

#[test]
fn test_set_and_get_values() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let entity = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    let entity_id = entity.entity_id();
    dojo::model::Model::<
        Foo
    >::set_entity(world, entity.keys(), entity.values(), entity.instance_layout());

    let mut entity_values = FooModelValues::get(world, entity_id);
    assert!(entity.v1 == entity_values.v1 && entity.v2 == entity_values.v2);

    entity_values.v1 = 12;
    entity_values.v2 = 18;

    entity_values.set(world, entity_id);

    let read_values = FooModelValues::get(world, entity_id);
    assert!(read_values.v1 == entity_values.v1 && read_values.v2 == entity_values.v2);
}

#[test]
fn test_entity_and_set_entity() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let entity = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    dojo::model::Model::<
        Foo
    >::set_entity(world, entity.keys(), entity.values(), entity.instance_layout());
    let read_entity = dojo::model::Model::<
        Foo
    >::entity(world, entity.keys(), entity.instance_layout());

    assert!(
        entity.k1 == read_entity.k1
            && entity.k2 == read_entity.k2
            && entity.v1 == read_entity.v1
            && entity.v2 == read_entity.v2
    );
}
