use async_graphql::dynamic::Schema;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use serde::Deserialize;
use serde_json::Value;
use sqlx::SqlitePool;
use starknet_crypto::FieldElement;
use tokio_stream::StreamExt;
use torii_core::sql::Sql;

mod entities_test;
mod metadata_test;
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
// same as type from `async-graphql` but derive necessary traits
// https://docs.rs/async-graphql/6.0.10/async_graphql/types/connection/struct.PageInfo.html
pub struct PageInfo {
    pub has_previous_page: bool,
    pub has_next_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Record {
    pub __typename: String,
    pub depth: String,
    pub record_id: u32,
    pub type_u8: u8,
    pub type_u16: u16,
    pub type_u32: u32,
    pub type_u64: u64,
    pub type_u128: String,
    pub type_u256: String,
    pub type_bool: bool,
    pub type_felt: String,
    pub type_class_hash: String,
    pub type_contract_address: String,
    pub random_u8: u8,
    pub random_u128: String,
    pub type_nested: Option<Nested>,
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
    pub type_nested_more_more: NestedMoreMore,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct NestedMoreMore {
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
        vec![],
        FieldElement::ONE,
        0,
        0,
    )
    .await
    .unwrap();
}
