use std::str::FromStr;

use anyhow::Result;
use async_graphql::dynamic::Schema;
use camino::Utf8PathBuf;
use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::{copy_types_test_db, prepare_migration_with_world_and_seed};
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use dojo_world::contracts::abi::model::Layout;
use dojo_world::contracts::WorldContractReader;
use dojo_world::utils::TransactionWaiter;
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use scarb::compiler::Profile;
use serde::Deserialize;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::types::{Felt, InvokeTransactionResult};
use starknet::core::utils::get_contract_address;
use starknet::macros::selector;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use torii_core::engine::{Engine, EngineConfig, Processors};
use torii_core::processors::register_model::RegisterModelProcessor;
use torii_core::processors::store_del_record::StoreDelRecordProcessor;
use torii_core::processors::store_set_record::StoreSetRecordProcessor;
use torii_core::sql::Sql;

mod entities_test;
mod events_test;
mod metadata_test;
mod models_ordering_test;
mod models_test;
mod subscription_test;

use crate::schema::build_schema;

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Connection<T> {
    pub total_count: i64,
    pub edges: Vec<Edge<T>>,
    pub page_info: PageInfo,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Edge<T> {
    pub node: T,
    pub cursor: String,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub keys: Option<Vec<String>>,
    pub created_at: Option<String>,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub keys: Vec<String>,
    pub data: Vec<String>,
    pub transaction_hash: String,
    pub executed_at: String,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
// same as type from `async-graphql` but derive necessary traits
// https://docs.rs/async-graphql/6.0.10/async_graphql/types/connection/struct.PageInfo.html
pub struct PageInfo {
    pub has_previous_page: bool,
    pub has_next_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldModel {
    pub id: String,
    pub name: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub created_at: String,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Record {
    pub __typename: String,
    pub depth: String,
    pub record_id: u32,
    pub type_u8: u8,
    pub type_u16: u16,
    pub type_u32: u32,
    pub type_u64: String,
    pub type_u128: String,
    pub type_u256: String,
    pub type_bool: bool,
    pub type_felt: String,
    pub type_class_hash: String,
    pub type_contract_address: String,
    pub random_u8: u8,
    pub random_u128: String,
    pub type_deeply_nested: Option<Nested>,
    pub type_nested_one: Option<NestedMost>,
    pub type_nested_two: Option<NestedMost>,
    pub entity: Option<Entity>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Nested {
    pub __typename: String,
    pub depth: String,
    pub type_number: u8,
    pub type_string: String,
    pub type_nested_more: NestedMore,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct NestedMore {
    pub __typename: String,
    pub depth: String,
    pub type_number: u8,
    pub type_string: String,
    pub type_nested_most: NestedMost,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct NestedMost {
    pub __typename: String,
    pub depth: String,
    pub type_number: u8,
    pub type_string: String,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Subrecord {
    pub __typename: String,
    pub record_id: u32,
    pub subrecord_id: u32,
    pub type_u8: u8,
    pub random_u8: u8,
    pub entity: Option<Entity>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct RecordSibling {
    pub __typename: String,
    pub record_id: u32,
    pub random_u8: u8,
    pub entity: Option<Entity>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Social {
    pub name: String,
    pub url: String,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    pub name: Option<String>,
    pub description: Option<String>,
    pub website: Option<String>,
    pub icon_uri: Option<String>,
    pub cover_uri: Option<String>,
    pub socials: Vec<Social>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub uri: String,
    pub world_address: String,
    pub icon_img: String,
    pub cover_img: String,
    pub content: Content,
}

pub async fn run_graphql_query(schema: &Schema, query: &str) -> Value {
    let res = schema.execute(query).await;

    println!("Trying to execute query: {}", query);

    assert!(res.errors.is_empty(), "GraphQL query returned errors: {:?}", res.errors);
    serde_json::to_value(res.data).expect("Failed to serialize GraphQL response")
}

#[allow(dead_code)]
pub async fn run_graphql_subscription(
    pool: &SqlitePool,
    subscription: &str,
) -> async_graphql::Value {
    // Build dynamic schema
    let schema = build_schema(pool).await.unwrap();
    schema.execute_stream(subscription).next().await.unwrap().into_result().unwrap().data
    // fn subscribe() is called from inside dynamic subscription
}

pub async fn model_fixtures(db: &mut Sql) {
    db.register_model(
        "types_test",
        Ty::Struct(Struct {
            name: "Record".to_string(),
            children: vec![
                Member {
                    name: "depth".to_string(),
                    key: false,
                    ty: Ty::Enum(Enum {
                        name: "Depth".to_string(),
                        option: None,
                        options: vec![
                            EnumOption { name: "Zero".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption { name: "One".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption { name: "Two".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption { name: "Three".to_string(), ty: Ty::Tuple(vec![]) },
                        ],
                    }),
                },
                Member {
                    name: "record_id".to_string(),
                    key: true,
                    ty: Ty::Primitive(Primitive::U32(None)),
                },
                Member {
                    name: "typeU16".to_string(),
                    key: false,
                    ty: Ty::Primitive(Primitive::U16(None)),
                },
                Member {
                    name: "type_u64".to_string(),
                    key: false,
                    ty: Ty::Primitive(Primitive::U64(None)),
                },
                Member {
                    name: "typeBool".to_string(),
                    key: false,
                    ty: Ty::Primitive(Primitive::Bool(None)),
                },
                Member {
                    name: "type_felt".to_string(),
                    key: false,
                    ty: Ty::Primitive(Primitive::Felt252(None)),
                },
                Member {
                    name: "typeContractAddress".to_string(),
                    key: true,
                    ty: Ty::Primitive(Primitive::ContractAddress(None)),
                },
            ],
        }),
        Layout::Fixed(vec![]),
        Felt::ONE,
        Felt::TWO,
        0,
        0,
        1710754478_u64,
    )
    .await
    .unwrap();
}

pub async fn spinup_types_test() -> Result<SqlitePool> {
    // change sqlite::memory: to sqlite:~/.test.db to dump database to disk
    let options =
        SqliteConnectOptions::from_str("sqlite::memory:")?.create_if_missing(true).with_regexp();
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let setup = CompilerTestSetup::from_paths("../../dojo-core", &["../types-test"]);
    let config = setup.build_test_config("types-test", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let manifest_path = Utf8PathBuf::from(config.manifest_path().parent().unwrap());
    let target_dir = Utf8PathBuf::from(ws.target_dir().to_string()).join("dev");

    let seq_config = KatanaRunnerConfig::default().with_db_dir(copy_types_test_db().as_str());
    let sequencer = KatanaRunner::new_with_config(seq_config).expect("Failed to start runner.");

    let account = sequencer.account(0);

    let (strat, _) = prepare_migration_with_world_and_seed(
        manifest_path,
        target_dir,
        None,
        "types_test",
        "types_test",
    )
    .unwrap();

    let records = strat.contracts.first().unwrap();
    let records_address = get_contract_address(
        records.salt,
        strat.base.as_ref().unwrap().diff.local_class_hash,
        &[],
        strat.world_address,
    );

    let InvokeTransactionResult { transaction_hash } = account
        .execute_v1(vec![Call {
            calldata: vec![Felt::from_str("0xa").unwrap()],
            to: records_address,
            selector: selector!("create"),
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(transaction_hash, &account.provider()).await?;

    // Execute `delete` and delete Record with id 20
    let InvokeTransactionResult { transaction_hash } = account
        .execute_v1(vec![Call {
            calldata: vec![Felt::from_str("0x14").unwrap()],
            to: records_address,
            selector: selector!("delete"),
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(transaction_hash, &account.provider()).await?;

    let world = WorldContractReader::new(strat.world_address, account.provider());

    let db = Sql::new(pool.clone(), strat.world_address, Felt::ZERO).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let mut engine = Engine::new(
        world,
        db,
        account.provider(),
        Processors {
            event: vec![
                Box::new(RegisterModelProcessor),
                Box::new(StoreSetRecordProcessor),
                Box::new(StoreDelRecordProcessor),
            ],
            ..Processors::default()
        },
        EngineConfig::default(),
        shutdown_tx,
        None,
    );

    let _ = engine.sync_to_head(0, None).await?;

    Ok(pool)
}
