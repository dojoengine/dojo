use camino::Utf8PathBuf;
use dojo_types::component::{Member, Struct, Ty};
use dojo_world::manifest::System;
use sqlx::sqlite::SqlitePool;
use starknet::core::types::{Event, FieldElement};

use crate::sql::{Executable, Sql};

#[sqlx::test(migrations = "../migrations")]
async fn test_load_from_manifest(pool: SqlitePool) {
    let manifest = dojo_world::manifest::Manifest::load_from_path(
        Utf8PathBuf::from_path_buf("../../../examples/ecs/target/dev/manifest.json".into())
            .unwrap(),
    )
    .unwrap();

    let state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
    state.load_from_manifest(manifest.clone()).await.unwrap();

    let models = sqlx::query("SELECT * FROM models").fetch_all(&pool).await.unwrap();
    assert_eq!(models.len(), 0);

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
        .register_model(
            Ty::Struct(Struct {
                name: "Position".into(),
                children: vec![Member {
                    name: "test".into(),
                    ty: Ty::Terminal("u32".to_string()),
                    key: false,
                }],
            }),
            vec![],
            FieldElement::TWO,
        )
        .await
        .unwrap();
    state.execute().await.unwrap();

    let (id, name, class_hash): (String, String, String) =
        sqlx::query_as("SELECT id, name, class_hash FROM models WHERE id = 'Position'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(id, "Position");
    assert_eq!(name, "Position");
    assert_eq!(class_hash, format!("{:#x}", FieldElement::TWO));

    let position_models = sqlx::query("SELECT * FROM [Position]").fetch_all(&pool).await.unwrap();
    assert_eq!(position_models.len(), 0);

    state
        .register_system(System {
            name: "Position".into(),
            inputs: vec![],
            outputs: vec![],
            class_hash: FieldElement::THREE,
            dependencies: vec![],
            ..Default::default()
        })
        .await
        .unwrap();
    state.execute().await.unwrap();

    let (id, name, class_hash): (String, String, String) =
        sqlx::query_as("SELECT id, name, class_hash FROM systems WHERE id = 'Position'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(id, "Position");
    assert_eq!(name, "Position");
    assert_eq!(class_hash, format!("{:#x}", FieldElement::THREE));

    state
        .set_entity(
            "Position".to_string(),
            vec![FieldElement::ONE],
            vec![
                FieldElement::ONE,
                FieldElement::from_dec_str("42").unwrap(),
                FieldElement::from_dec_str("69").unwrap(),
            ],
        )
        .await
        .unwrap();

    // state
    //     .store_system_call(
    //         "Test".into(),
    //         FieldElement::from_str("0x4").unwrap(),
    //         &[FieldElement::ONE, FieldElement::TWO, FieldElement::THREE],
    //     )
    //     .await
    //     .unwrap();

    state
        .store_event(
            &Event {
                from_address: FieldElement::ONE,
                keys: Vec::from([FieldElement::TWO]),
                data: Vec::from([FieldElement::TWO, FieldElement::THREE]),
            },
            0,
            FieldElement::THREE,
        )
        .await
        .unwrap();

    state.execute().await.unwrap();

    let keys = format!("{:#x}/", FieldElement::TWO);
    let query = format!("SELECT data, transaction_hash FROM events WHERE keys = '{}'", keys);
    let (data, tx_hash): (String, String) = sqlx::query_as(&query).fetch_one(&pool).await.unwrap();

    assert_eq!(data, format!("{:#x}/{:#x}/", FieldElement::TWO, FieldElement::THREE));
    assert_eq!(tx_hash, format!("{:#x}", FieldElement::THREE))
}
