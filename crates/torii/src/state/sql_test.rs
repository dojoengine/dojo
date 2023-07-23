use camino::Utf8PathBuf;
use dojo_types::component::Member;
use dojo_world::manifest::{Component, System};
use sqlx::sqlite::SqlitePool;
use starknet::core::types::FieldElement;

use crate::state::sql::{Executable, Sql};
use crate::state::State;

#[sqlx::test(migrations = "./migrations")]
async fn test_load_from_manifest(pool: SqlitePool) {
    let manifest = dojo_world::manifest::Manifest::load_from_path(
        Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev/manifest.json".into()).unwrap(),
    )
    .unwrap();

    let state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
    state.load_from_manifest(manifest.clone()).await.unwrap();

    let components = sqlx::query("SELECT * FROM components").fetch_all(&pool).await.unwrap();
    assert_eq!(components.len(), 2);

    let moves_components =
        sqlx::query("SELECT * FROM external_moves").fetch_all(&pool).await.unwrap();
    assert_eq!(moves_components.len(), 0);

    let systems = sqlx::query("SELECT * FROM systems").fetch_all(&pool).await.unwrap();
    assert_eq!(systems.len(), 3);

    let mut world = state.world().await.unwrap();

    assert_eq!(world.world_address.0, FieldElement::ZERO);
    assert_eq!(world.world_class_hash.0, manifest.world.class_hash);
    assert_eq!(world.executor_address.0, FieldElement::ZERO);
    assert_eq!(world.executor_class_hash.0, manifest.executor.class_hash);

    world.executor_address.0 = FieldElement::ONE;
    state.set_world(world).await.unwrap();
    state.execute().await.unwrap();

    let world = state.world().await.unwrap();
    assert_eq!(world.executor_address.0, FieldElement::ONE);

    let head = state.head().await.unwrap();
    assert_eq!(head, 0);

    state.set_head(1).await.unwrap();
    state.execute().await.unwrap();

    let head = state.head().await.unwrap();
    assert_eq!(head, 1);

    state
        .register_component(Component {
            name: "Test".into(),
            members: vec![Member { name: "test".into(), ty: "u32".into(), slot: 0, offset: 1 }],
            class_hash: FieldElement::TWO,
        })
        .await
        .unwrap();
    state.execute().await.unwrap();

    let (id, name, class_hash): (String, String, String) =
        sqlx::query_as("SELECT id, name, class_hash FROM components WHERE id = 'test'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(id, "test");
    assert_eq!(name, "Test");
    assert_eq!(class_hash, format!("{:#x}", FieldElement::TWO));

    let test_components =
        sqlx::query("SELECT * FROM external_test").fetch_all(&pool).await.unwrap();
    assert_eq!(test_components.len(), 0);

    state
        .register_system(System {
            name: "Test".into(),
            inputs: vec![],
            outputs: vec![],
            class_hash: FieldElement::THREE,
            dependencies: vec![],
        })
        .await
        .unwrap();
    state.execute().await.unwrap();

    let (id, name, class_hash): (String, String, String) =
        sqlx::query_as("SELECT id, name, class_hash FROM systems WHERE id = 'test'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(id, "test");
    assert_eq!(name, "Test");
    assert_eq!(class_hash, format!("{:#x}", FieldElement::THREE));

    state
        .set_entity(
            "Position".to_string(),
            FieldElement::ZERO,
            vec![FieldElement::ONE],
            vec![
                FieldElement::from_dec_str("42").unwrap(),
                FieldElement::from_dec_str("69").unwrap(),
            ],
        )
        .await
        .unwrap();
    state.execute().await.unwrap();
}
