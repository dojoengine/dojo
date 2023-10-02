use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, Member, Struct, Ty};
use serde::Deserialize;
use serde_json::Value;
use sqlx::SqlitePool;
use starknet::core::types::FieldElement;
use tokio_stream::StreamExt;
use torii_core::sql::Sql;

mod entities_test;
// mod models_test;
// mod subscription_test;

use crate::schema::build_schema;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Connection<T> {
    pub total_count: i64,
    pub edges: Vec<Edge<T>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Edge<T> {
    pub node: T,
    pub cursor: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub model_names: String,
    pub keys: Option<Vec<String>>,
    pub created_at: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Moves {
    pub __typename: String,
    pub remaining: u32,
    pub last_direction: u8,
    pub entity: Option<Entity>,
}

#[derive(Deserialize, Debug)]
pub struct Position {
    pub __typename: String,
    pub x: u32,
    pub y: u32,
    pub entity: Option<Entity>,
}

pub enum Paginate {
    Forward,
    Backward,
}

pub async fn run_graphql_query(pool: &SqlitePool, query: &str) -> Value {
    let schema = build_schema(pool).await.unwrap();
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

pub async fn entity_fixtures(db: &mut Sql) {
    db.register_model(
        Ty::Struct(Struct {
            name: "Moves".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    key: true,
                    ty: Ty::Primitive(Primitive::ContractAddress(None)),
                },
                Member {
                    name: "remaining".to_string(),
                    key: false,
                    ty: Ty::Primitive(Primitive::U8(None)),
                },
                Member {
                    name: "last_direction".to_string(),
                    key: false,
                    ty: Ty::Enum(Enum {
                        name: "Direction".to_string(),
                        option: None,
                        options: vec![
                            ("None".to_string(), Ty::Tuple(vec![])),
                            ("Left".to_string(), Ty::Tuple(vec![])),
                            ("Right".to_string(), Ty::Tuple(vec![])),
                            ("Up".to_string(), Ty::Tuple(vec![])),
                            ("Down".to_string(), Ty::Tuple(vec![])),
                        ],
                    }),
                },
            ],
        }),
        vec![],
        FieldElement::ONE,
    )
    .await
    .unwrap();

    db.register_model(
        Ty::Struct(Struct {
            name: "Position".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    key: true,
                    ty: Ty::Primitive(Primitive::ContractAddress(None)),
                },
                Member {
                    name: "vec".to_string(),
                    key: false,
                    ty: Ty::Struct(Struct {
                        name: "Vec2".to_string(),
                        children: vec![
                            Member {
                                name: "x".to_string(),
                                key: false,
                                ty: Ty::Primitive(Primitive::U32(None)),
                            },
                            Member {
                                name: "y".to_string(),
                                key: false,
                                ty: Ty::Primitive(Primitive::U32(None)),
                            },
                        ],
                    }),
                },
            ],
        }),
        vec![],
        FieldElement::TWO,
    )
    .await
    .unwrap();

    db.set_entity(Ty::Struct(Struct {
        name: "Moves".to_string(),
        children: vec![
            Member {
                name: "player".to_string(),
                key: true,
                ty: Ty::Primitive(Primitive::ContractAddress(Some(FieldElement::ONE))),
            },
            Member {
                name: "remaining".to_string(),
                key: false,
                ty: Ty::Primitive(Primitive::U8(Some(10))),
            },
            Member {
                name: "last_direction".to_string(),
                key: false,
                ty: Ty::Enum(Enum {
                    name: "Direction".to_string(),
                    option: Some(1),
                    options: vec![
                        ("None".to_string(), Ty::Tuple(vec![])),
                        ("Left".to_string(), Ty::Tuple(vec![])),
                        ("Right".to_string(), Ty::Tuple(vec![])),
                        ("Up".to_string(), Ty::Tuple(vec![])),
                        ("Down".to_string(), Ty::Tuple(vec![])),
                    ],
                }),
            },
        ],
    }))
    .await
    .unwrap();

    db.set_entity(Ty::Struct(Struct {
        name: "Position".to_string(),
        children: vec![
            Member {
                name: "player".to_string(),
                key: true,
                ty: Ty::Primitive(Primitive::ContractAddress(Some(FieldElement::TWO))),
            },
            Member {
                name: "vec".to_string(),
                key: false,
                ty: Ty::Struct(Struct {
                    name: "Vec2".to_string(),
                    children: vec![
                        Member {
                            name: "x".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::U32(Some(42))),
                        },
                        Member {
                            name: "y".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::U32(Some(69))),
                        },
                    ],
                }),
            },
        ],
    }))
    .await
    .unwrap();

    // Set an entity with both moves and position models
    db.set_entity(Ty::Struct(Struct {
        name: "Moves".to_string(),
        children: vec![
            Member {
                name: "player".to_string(),
                key: true,
                ty: Ty::Primitive(Primitive::ContractAddress(Some(FieldElement::THREE))),
            },
            Member {
                name: "remaining".to_string(),
                key: false,
                ty: Ty::Primitive(Primitive::U8(Some(10))),
            },
            Member {
                name: "last_direction".to_string(),
                key: false,
                ty: Ty::Enum(Enum {
                    name: "Direction".to_string(),
                    option: Some(2),
                    options: vec![
                        ("None".to_string(), Ty::Tuple(vec![])),
                        ("Left".to_string(), Ty::Tuple(vec![])),
                        ("Right".to_string(), Ty::Tuple(vec![])),
                        ("Up".to_string(), Ty::Tuple(vec![])),
                        ("Down".to_string(), Ty::Tuple(vec![])),
                    ],
                }),
            },
        ],
    }))
    .await
    .unwrap();

    db.set_entity(Ty::Struct(Struct {
        name: "Position".to_string(),
        children: vec![
            Member {
                name: "player".to_string(),
                key: true,
                ty: Ty::Primitive(Primitive::ContractAddress(Some(FieldElement::THREE))),
            },
            Member {
                name: "vec".to_string(),
                key: false,
                ty: Ty::Struct(Struct {
                    name: "Vec2".to_string(),
                    children: vec![
                        Member {
                            name: "x".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::U32(Some(42))),
                        },
                        Member {
                            name: "y".to_string(),
                            key: false,
                            ty: Ty::Primitive(Primitive::U32(Some(69))),
                        },
                    ],
                }),
            },
        ],
    }))
    .await
    .unwrap();

    db.execute().await.unwrap();
}

pub async fn paginate(
    pool: &SqlitePool,
    cursor: Option<String>,
    direction: Paginate,
    page_size: usize,
) -> Connection<Entity> {
    let (first_last, before_after) = match direction {
        Paginate::Forward => ("first", "after"),
        Paginate::Backward => ("last", "before"),
    };

    let cursor = cursor.map_or(String::new(), |c| format!(", {before_after}: \"{c}\""));
    let query = format!(
        "
        {{
            entities ({first_last}: {page_size} {cursor}) 
            {{
                totalCount
                edges {{
                    cursor
                    node {{
                        modelNames
                    }}
                }}
            }}
        }}
        "
    );

    let value = run_graphql_query(pool, &query).await;
    let entities = value.get("entities").ok_or("entities not found").unwrap();
    serde_json::from_value(entities.clone()).unwrap()
}
