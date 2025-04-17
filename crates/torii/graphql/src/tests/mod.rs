use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use async_graphql::dynamic::Schema;
use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::copy_types_test_db;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use dojo_utils::{TransactionExt, TransactionWaiter, TxnConfig};
use dojo_world::contracts::abigen::model::Layout;
use dojo_world::contracts::abigen::world::Resource;
use dojo_world::contracts::naming::{compute_bytearray_hash, compute_selector_from_tag};
use dojo_world::contracts::{WorldContract, WorldContractReader};
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use scarb::compiler::Profile;
use serde::Deserialize;
use serde_json::Value;
use sozo_scarbext::WorkspaceExt;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{Call, Felt, InvokeTransactionResult};
use starknet::macros::selector;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use torii_indexer::engine::{Engine, EngineConfig, Processors};
use torii_sqlite::cache::ModelCache;
use torii_sqlite::executor::Executor;
use torii_sqlite::types::{Contract, ContractType};
use torii_sqlite::Sql;

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
        &Ty::Struct(Struct {
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
        None,
    )
    .await
    .unwrap();

    db.execute().await.unwrap();
}

pub async fn spinup_types_test(path: &str) -> Result<SqlitePool> {
    let options =
        SqliteConnectOptions::from_str(path).unwrap().create_if_missing(true).with_regexp();
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let setup = CompilerTestSetup::from_paths("../../dojo/core", &["../types-test"]);
    let config = setup.build_test_config("types-test", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let seq_config = KatanaRunnerConfig { n_accounts: 10, ..Default::default() }
        .with_db_dir(copy_types_test_db().as_str());

    let sequencer = KatanaRunner::new_with_config(seq_config).expect("Failed to start runner.");

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();

    let world = WorldContract::new(world_address, &account);

    let records_address = if let Resource::Contract((records_address, _)) =
        world.resource(&compute_selector_from_tag("types_test-records")).call().await.unwrap()
    {
        records_address
    } else {
        panic!("Failed to get records address")
    };

    world
        .grant_writer(&compute_bytearray_hash("types_test"), &records_address)
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let InvokeTransactionResult { transaction_hash } = account
        .execute_v3(vec![Call {
            calldata: vec![Felt::from_str("0xa").unwrap()],
            to: records_address.into(),
            selector: selector!("create"),
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(transaction_hash, &account.provider()).await?;

    // Execute `delete` and delete Record with id 20
    let InvokeTransactionResult { transaction_hash } = account
        .execute_v3(vec![Call {
            calldata: vec![Felt::from_str("0x14").unwrap()],
            to: records_address.into(),
            selector: selector!("delete"),
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(transaction_hash, &provider).await?;

    let world = WorldContractReader::new(world_address, Arc::clone(&provider));

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(
        pool.clone(),
        sender,
        &[Contract { address: world_address, r#type: ContractType::WORLD }],
        model_cache,
    )
    .await
    .unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let mut engine = Engine::new(
        world,
        db.clone(),
        Arc::clone(&provider),
        Processors { ..Processors::default() },
        EngineConfig::default(),
        shutdown_tx,
        None,
        &[Contract { address: world_address, r#type: ContractType::WORLD }],
    );

    let to = account.provider().block_hash_and_number().await?.block_number;
    let data = engine.fetch_range(0, to, &HashMap::new(), to).await.unwrap();
    engine.process_range(data).await.unwrap();
    db.execute().await.unwrap();
    Ok(pool)
}
