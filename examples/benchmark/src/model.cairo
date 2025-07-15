use dojo::model::{Model, ModelStorage, ModelValueStorage, ModelDefinition};
use dojo::world::WorldStorage;
use dojo_cairo_test::{NamespaceDef, TestResource, spawn_test_world};

#[derive(Copy, Drop, Serde)]
#[dojo::model]
struct Single {
    #[key]
    k0: felt252,
    v0: felt252,
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
struct Large {
    #[key]
    k0: felt252,
    #[key]
    k1: felt252,
    v0: felt252,
    v1: felt252,
    v2: felt252,
    v3: felt252,
    v4: felt252,
    v5: felt252,
}

const SINGLE: Single = Single { k0: 1, v0: 2 };
const LARGE: Large = Large { k0: 1, k1: 2, v0: 3, v1: 4, v2: 5, v3: 6, v4: 7, v5: 8 };

#[derive(Copy, Drop, Serde, Introspect)]
struct SingleSchema {
    v0: felt252,
}

#[derive(Copy, Drop, Serde, Introspect)]
struct LargeSingleSchema {
    v5: felt252,
}

#[derive(Copy, Drop, Serde, Introspect)]
struct LargeDoubleSchema {
    v0: felt252,
    v5: felt252,
}

#[derive(Copy, Drop, Serde, Introspect)]
struct LargeSextupleSchema {
    v0: felt252,
    v1: felt252,
    v2: felt252,
    v3: felt252,
    v4: felt252,
    v5: felt252,
}


fn namespace_def() -> NamespaceDef {
    NamespaceDef {
        namespace: "dojo_cairo_test",
        resources: [
            TestResource::Model(m_Single::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Model(m_Large::TEST_CLASS_HASH.try_into().unwrap()),
        ]
            .span(),
    }
}

fn spawn_foo_world() -> WorldStorage {
    spawn_test_world([namespace_def()].span())
}

#[test]
fn read_model_simple() {
    let mut world = spawn_foo_world();
    world.write_model(@SINGLE);

    let model: Single = world.read_model(SINGLE.k0);
    assert!(model.v0 == SINGLE.v0);
}

#[test]
fn read_value_simple() {
    let mut world = spawn_foo_world();
    world.write_model(@SINGLE);

    let value: SingleValue = world.read_value(SINGLE.k0);
    assert!(value.v0 == SINGLE.v0);
}


#[test]
fn read_schema_simple() {
    let mut world = spawn_foo_world();
    world.write_model(@SINGLE);

    let schema: SingleSchema = world.read_schema(SINGLE.ptr());
    assert!(schema.v0 == SINGLE.v0);
}

#[test]
fn read_member_simple() {
    let mut world = spawn_foo_world();
    world.write_model(@SINGLE);

    let v0: felt252 = world.read_member(SINGLE.ptr(), selector!("v0"));
    assert!(v0 == SINGLE.v0);
}


#[test]
fn read_single_model_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let model: Large = world.read_model((LARGE.k0, LARGE.k1));
    assert!(model.v5 == LARGE.v5);
}

#[test]
fn read_single_value_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let value: LargeValue = world.read_value((LARGE.k0, LARGE.k1));
    assert!(value.v5 == LARGE.v5);
}

#[test]
fn read_single_schema_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let schema: LargeSingleSchema = world.read_schema(LARGE.ptr());
    assert!(schema.v5 == LARGE.v5);
}

#[test]
fn read_single_member_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let v5: felt252 = world.read_member(LARGE.ptr(), selector!("v5"));
    assert!(v5 == LARGE.v5);
}


#[test]
fn read_double_model_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let model: Large = world.read_model((LARGE.k0, LARGE.k1));
    assert!(model.v0 == LARGE.v0 && model.v5 == LARGE.v5);
}

#[test]
fn read_double_value_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let value: LargeValue = world.read_value((LARGE.k0, LARGE.k1));
    assert!(value.v0 == LARGE.v0 && value.v5 == LARGE.v5);
}

#[test]
fn read_double_schema_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let schema: LargeDoubleSchema = world.read_schema(LARGE.ptr());
    assert!(schema.v0 == LARGE.v0 && schema.v5 == LARGE.v5);
}

#[test]
fn read_double_member_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let v0: felt252 = world.read_member(LARGE.ptr(), selector!("v0"));
    let v5: felt252 = world.read_member(LARGE.ptr(), selector!("v5"));
    assert!(v0 == LARGE.v0 && v5 == LARGE.v5);
}

#[test]
fn read_sextuple_model_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let model: Large = world.read_model((LARGE.k0, LARGE.k1));
    assert!(
        model.v0 == LARGE.v0
            && model.v1 == LARGE.v1
            && model.v2 == LARGE.v2
            && model.v3 == LARGE.v3
            && model.v4 == LARGE.v4
            && model.v5 == LARGE.v5,
    );
}

#[test]
fn read_sextuple_value_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let value: LargeValue = world.read_value((LARGE.k0, LARGE.k1));
    assert!(
        value.v0 == LARGE.v0
            && value.v1 == LARGE.v1
            && value.v2 == LARGE.v2
            && value.v3 == LARGE.v3
            && value.v4 == LARGE.v4
            && value.v5 == LARGE.v5,
    );
}

#[test]
fn read_sextuple_schema_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);

    let schema: LargeSextupleSchema = world.read_schema(LARGE.ptr());
    assert!(
        schema.v0 == LARGE.v0
            && schema.v1 == LARGE.v1
            && schema.v2 == LARGE.v2
            && schema.v3 == LARGE.v3
            && schema.v4 == LARGE.v4
            && schema.v5 == LARGE.v5,
    );
}

#[test]
fn read_sextuple_member_large() {
    let mut world = spawn_foo_world();
    world.write_model(@LARGE);
    let v0: felt252 = world.read_member(LARGE.ptr(), selector!("v0"));
    let v1: felt252 = world.read_member(LARGE.ptr(), selector!("v1"));
    let v2: felt252 = world.read_member(LARGE.ptr(), selector!("v2"));
    let v3: felt252 = world.read_member(LARGE.ptr(), selector!("v3"));
    let v4: felt252 = world.read_member(LARGE.ptr(), selector!("v4"));
    let v5: felt252 = world.read_member(LARGE.ptr(), selector!("v5"));
    assert!(
        v0 == LARGE.v0
            && v1 == LARGE.v1
            && v2 == LARGE.v2
            && v3 == LARGE.v3
            && v4 == LARGE.v4
            && v5 == LARGE.v5,
    );
}
