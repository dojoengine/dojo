use dojo::model::ModelStorage;
use dojo_snf_test::world::{NamespaceDef, TestResource, spawn_test_world};
use crate::tests::benches::bench_data::*;

#[test]
fn bench_read_model_small() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("SmallModel")].span() }]
            .span(),
    );
    world.write_model(@build_small_model());
    let _m: SmallModel = world.read_model(get_model_keys());
}

#[test]
fn bench_read_model_medium() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("MediumModel")].span() }]
            .span(),
    );
    world.write_model(@build_medium_model());
    let _m: MediumModel = world.read_model(get_model_keys());
}

#[test]
fn bench_read_model_big() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("BigModel")].span() }]
            .span(),
    );
    world.write_model(@build_big_model());
    let _m: BigModel = world.read_model(get_model_keys());
}

#[test]
fn bench_read_models_small() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("SmallModel")].span() }]
            .span(),
    );
    let keys = get_model_keys();
    world.write_model(@build_small_model());
    let _m: Array<SmallModel> = world.read_models(array![keys, keys, keys, keys, keys].span());
}

#[test]
fn bench_read_models_medium() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("MediumModel")].span() }]
            .span(),
    );
    let keys = get_model_keys();
    world.write_model(@build_medium_model());
    let _m: Array<MediumModel> = world.read_models(array![keys, keys, keys, keys, keys].span());
}

#[test]
fn bench_read_models_big() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("BigModel")].span() }]
            .span(),
    );
    let keys = get_model_keys();
    world.write_model(@build_big_model());
    let _m: Array<BigModel> = world.read_models(array![keys, keys, keys, keys, keys].span());
}

#[test]
fn bench_write_model_small() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("SmallModel")].span() }]
            .span(),
    );
    world.write_model(@build_small_model());
}

#[test]
fn bench_write_model_medium() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("MediumModel")].span() }]
            .span(),
    );
    world.write_model(@build_medium_model());
}

#[test]
fn bench_write_model_big() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("BigModel")].span() }]
            .span(),
    );
    world.write_model(@build_big_model());
}

#[test]
fn bench_erase_model_small() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("SmallModel")].span() }]
            .span(),
    );
    let model = build_small_model();
    world.write_model(@model);
    world.erase_model(@model);
}

#[test]
fn bench_erase_model_medium() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("MediumModel")].span() }]
            .span(),
    );
    let model = build_medium_model();
    world.write_model(@model);
    world.erase_model(@model);
}

#[test]
fn bench_erase_model_big() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("BigModel")].span() }]
            .span(),
    );
    let model = build_big_model();
    world.write_model(@model);
    world.erase_model(@model);
}

#[test]
fn bench_write_models_small() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("SmallModel")].span() }]
            .span(),
    );
    let model = build_small_model();
    world.write_models(array![@model, @model, @model, @model, @model].span());
}

#[test]
fn bench_write_models_medium() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("MediumModel")].span() }]
            .span(),
    );
    let model = build_medium_model();
    world.write_models(array![@model, @model, @model, @model, @model].span());
}

#[test]
fn bench_write_models_big() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("BigModel")].span() }]
            .span(),
    );
    let model = build_big_model();
    world.write_models(array![@model, @model, @model, @model, @model].span());
}

#[test]
fn bench_erase_models_small() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("SmallModel")].span() }]
            .span(),
    );
    let model = build_small_model();
    world.erase_models(array![@model, @model, @model, @model, @model].span());
}

#[test]
fn bench_erase_models_medium() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("MediumModel")].span() }]
            .span(),
    );
    let model = build_medium_model();
    world.erase_models(array![@model, @model, @model, @model, @model].span());
}

#[test]
fn bench_erase_models_big() {
    let mut world = spawn_test_world(
        [NamespaceDef { namespace: "ns", resources: [TestResource::Model("BigModel")].span() }]
            .span(),
    );
    let model = build_big_model();
    world.erase_models(array![@model, @model, @model, @model, @model].span());
}

#[test]
fn bench_write_model_small_packed() {
    let mut world = spawn_test_world(
        [
            NamespaceDef {
                namespace: "ns", resources: [TestResource::Model("SmallPackedModel")].span(),
            }
        ]
            .span(),
    );
    world.write_model(@build_small_packed_model());
}

#[test]
fn bench_write_model_medium_packed() {
    let mut world = spawn_test_world(
        [
            NamespaceDef {
                namespace: "ns", resources: [TestResource::Model("MediumPackedModel")].span(),
            }
        ]
            .span(),
    );
    world.write_model(@build_medium_packed_model());
}

#[test]
fn bench_write_model_big_packed() {
    let mut world = spawn_test_world(
        [
            NamespaceDef {
                namespace: "ns", resources: [TestResource::Model("BigPackedModel")].span(),
            }
        ]
            .span(),
    );
    world.write_model(@build_big_packed_model());
}

#[test]
fn bench_erase_model_small_packed() {
    let mut world = spawn_test_world(
        [
            NamespaceDef {
                namespace: "ns", resources: [TestResource::Model("SmallPackedModel")].span(),
            }
        ]
            .span(),
    );
    let model = build_small_packed_model();
    world.write_model(@model);
    world.erase_model(@model);
}

#[test]
fn bench_erase_model_medium_packed() {
    let mut world = spawn_test_world(
        [
            NamespaceDef {
                namespace: "ns", resources: [TestResource::Model("MediumPackedModel")].span(),
            }
        ]
            .span(),
    );
    let model = build_medium_packed_model();
    world.write_model(@model);
    world.erase_model(@model);
}

#[test]
fn bench_erase_model_big_packed() {
    let mut world = spawn_test_world(
        [
            NamespaceDef {
                namespace: "ns", resources: [TestResource::Model("BigPackedModel")].span(),
            }
        ]
            .span(),
    );
    let model = build_big_packed_model();
    world.write_model(@model);
    world.erase_model(@model);
}
