use dojo::model::{Model, ModelValue, ModelStore};
use dojo::world::{IWorldDispatcherTrait};

use dojo::tests::helpers::{deploy_world};

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
fn test_id() {
    let mvalues = FooModelValue { __id: 1, v1: 3, v2: 4 };
    assert!(mvalues.id() == 1);
}

#[test]
fn test_values() {
    let mvalues = FooModelValue { __id: 1, v1: 3, v2: 4 };
    let expected_values = [3, 4].span();

    let values = mvalues.values();
    assert!(expected_values == values);
}

#[test]
fn test_from_values() {
    let mut values = [3, 4].span();

    let model_entity: Option<FooEntity> = Entity::from_values(1, ref values);
    assert!(model_entity.is_some());
    let model_entity = model_entity.unwrap();
    assert!(model_entity.__id == 1 && model_entity.v1 == 3 && model_entity.v2 == 4);
}

#[test]
fn test_from_values_bad_data() {
    let mut values = [3].span();
    let res: Option<FooEntity> = Entity::from_values(1, ref values);
    assert!(res.is_none());
}

#[test]
fn test_get_and_update_entity() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.set(@foo);

    let entity_id = foo.entity_id();
    let mut entity: FooEntity = world.get_entity(foo.key());
    assert_eq!(entity.__id, entity_id);
    assert_eq!(entity.v1, entity.v1);
    assert_eq!(entity.v2, entity.v2);

    entity.v1 = 12;
    entity.v2 = 18;

    world.update(@entity);

    let read_values: FooEntity = world.get_entity_from_id(entity_id);
    assert!(read_values.v1 == entity.v1 && read_values.v2 == entity.v2);
}

#[test]
fn test_delete_entity() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.set(@foo);

    let entity_id = foo.entity_id();
    let mut entity: FooEntity = world.get_entity_from_id(entity_id);
    EntityStore::delete_entity(world, @entity);

    let read_values: FooEntity = world.get_entity_from_id(entity_id);
    assert!(read_values.v1 == 0 && read_values.v2 == 0);
}

#[test]
fn test_get_and_set_member_from_entity() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.set(@foo);

    let v1: u128 = EntityStore::<
        FooEntity
    >::get_member_from_id(@world, foo.entity_id(), selector!("v1"));

    assert_eq!(v1, 3);

    let entity: FooEntity = world.get_entity_from_id(foo.entity_id());
    EntityStore::<FooEntity>::update_member_from_id(world, entity.id(), selector!("v1"), 42);

    let entity: FooEntity = world.get_entity_from_id(foo.entity_id());
    assert_eq!(entity.v1, 42);
}

#[test]
fn test_get_and_set_field_name() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.set(@foo);

    let v1 = FooMembersStore::get_v1_from_id(@world, foo.entity_id());
    assert!(foo.v1 == v1);

    let _entity: FooEntity = world.get_entity_from_id(foo.entity_id());

    FooMembersStore::update_v1_from_id(world, foo.entity_id(), 42);

    let v1 = FooMembersStore::get_v1_from_id(@world, foo.entity_id());
    assert!(v1 == 42);
}

#[test]
fn test_get_and_set_from_model() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.set(@foo);

    let read_entity: Foo = world.get((foo.k1, foo.k2));

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
    world.set(@foo);
    world.delete(@foo);

    let read_entity: Foo = world.get((foo.k1, foo.k2));
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
    world.set(@foo);
    let key: (u8, felt252) = foo.key();
    let v1: u128 = ModelStore::<Foo>::get_member(@world, key, selector!("v1"));

    assert!(v1 == 3);

    ModelStore::<Foo>::update_member(world, key, selector!("v1"), 42);
    let foo: Foo = world.get((foo.k1, foo.k2));
    assert!(foo.v1 == 42);
}

#[test]
fn test_get_and_set_field_name_from_model() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = Foo { k1: 1, k2: 2, v1: 3, v2: 4 };
    world.set(@foo);

    let v1 = FooMembersStore::get_v1(@world, (foo.k1, foo.k2));
    assert!(v1 == 3);

    FooMembersStore::update_v1(world, (foo.k1, foo.k2), 42);

    let v1 = FooMembersStore::get_v1(@world, (foo.k1, foo.k2));
    assert!(v1 == 42);
}

