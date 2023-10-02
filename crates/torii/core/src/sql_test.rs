use camino::Utf8PathBuf;
use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Member, Struct, Ty};
use dojo_world::manifest::System;
use dojo_world::migration::strategy::MigrationStrategy;
use scarb_ui::{OutputFormat, Ui, Verbosity};
use sozo::ops::migration::execute_strategy;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use starknet::core::types::{BlockId, BlockTag, Event, FieldElement};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use torii_client::contract::world::WorldContractReader;

use crate::engine::{Engine, EngineConfig, Processors};
use crate::processors::register_model::RegisterModelProcessor;
use crate::processors::register_system::RegisterSystemProcessor;
use crate::processors::store_set_record::StoreSetRecordProcessor;
use crate::sql::Sql;

pub async fn bootstrap_engine<'a>(
    world: &'a WorldContractReader<'a, JsonRpcClient<HttpTransport>>,
    db: &'a mut Sql,
    provider: &'a JsonRpcClient<HttpTransport>,
    migration: &MigrationStrategy,
    sequencer: &TestSequencer,
) -> Result<Engine<'a, JsonRpcClient<HttpTransport>>, Box<dyn std::error::Error>> {
    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let manifest = dojo_world::manifest::Manifest::load_from_path(
        Utf8PathBuf::from_path_buf("../../../examples/ecs/target/dev/manifest.json".into())
            .unwrap(),
    )
    .unwrap();

    db.load_from_manifest(manifest.clone()).await.unwrap();

    let ui = Ui::new(Verbosity::Verbose, OutputFormat::Text);
    execute_strategy(migration, &account, &ui, None).await.unwrap();

    let mut engine = Engine::new(
        world,
        db,
        provider,
        Processors {
            event: vec![
                Box::new(RegisterModelProcessor),
                Box::new(RegisterSystemProcessor),
                Box::new(StoreSetRecordProcessor),
            ],
            ..Processors::default()
        },
        EngineConfig::default(),
        None,
    );

    let _ = engine.sync_to_head(0).await?;

    Ok(engine)
}

#[sqlx::test(migrations = "../migrations")]
async fn test_load_from_manifest(pool: SqlitePool) {
    let manifest = dojo_world::manifest::Manifest::load_from_path(
        Utf8PathBuf::from_path_buf("../../../examples/ecs/target/dev/manifest.json".into())
            .unwrap(),
    )
    .unwrap();

    let mut state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
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
                children: vec![
                    Member {
                        name: "player".into(),
                        ty: Ty::Primitive(Primitive::ContractAddress(None)),
                        key: false,
                    },
                    Member {
                        name: "x".to_string(),
                        key: true,
                        ty: Ty::Primitive(Primitive::U32(None)),
                    },
                    Member {
                        name: "y".to_string(),
                        key: true,
                        ty: Ty::Primitive(Primitive::U32(None)),
                    },
                ],
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
        .set_entity(Ty::Struct(Struct {
            name: "Position".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    key: true,
                    ty: Ty::Primitive(Primitive::ContractAddress(Some(FieldElement::ONE))),
                },
                Member {
                    name: "x".to_string(),
                    key: true,
                    ty: Ty::Primitive(Primitive::U32(Some(42))),
                },
                Member {
                    name: "y".to_string(),
                    key: true,
                    ty: Ty::Primitive(Primitive::U32(Some(69))),
                },
            ],
        }))
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

#[tokio::test(flavor = "multi_thread")]
async fn test_load_from_remote() {
    let pool =
        SqlitePoolOptions::new().max_connections(5).connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();
    let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
    let migration = prepare_migration("../../../examples/ecs/target/dev".into()).unwrap();
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));
    let world = WorldContractReader::new(migration.world_address().unwrap(), &provider);

    let _ = bootstrap_engine(&world, &mut db, &provider, &migration, &sequencer).await;
}
