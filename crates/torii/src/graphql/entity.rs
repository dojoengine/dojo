use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, Result, Sqlite};

use super::types::ScalarType;
use super::{ObjectTraitInstance, ObjectTraitStatic, TypeMapping, ValueMapping};

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub partition_id: String,
    pub keys: Option<String>,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

pub struct EntityObject {
    pub field_type_mapping: TypeMapping,
}

impl ObjectTraitStatic for EntityObject {
    fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::ID),
                (Name::new("name"), TypeRef::STRING),
                (Name::new("partitionId"), ScalarType::FELT),
                (Name::new("keys"), TypeRef::STRING),
                (Name::new("transactionHash"), ScalarType::FELT),
                (Name::new("createdAt"), ScalarType::DATE_TIME),
            ]),
        }
    }
    fn from(field_type_mapping: TypeMapping) -> Self {
        Self { field_type_mapping }
    }
}

impl ObjectTraitInstance for EntityObject {
    fn name(&self) -> &str {
        "entity"
    }

    fn type_name(&self) -> &str {
        "Entity"
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn field_resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.string()?.replace('\"', "");
                    let entity_values = entity_by_id(&mut conn, &id).await?;
                    Ok(Some(FieldValue::owned_any(entity_values)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        ]
    }
}

async fn entity_by_id(conn: &mut PoolConnection<Sqlite>, id: &str) -> Result<ValueMapping> {
    let entity = sqlx::query_as!(
        Entity,
        r#"
            SELECT 
                id,
                name,
                partition_id,
                keys,
                transaction_hash,
                created_at as "created_at: _"
            FROM entities 
            WHERE id = $1
        "#,
        id,
    )
    .fetch_one(conn)
    .await?;

    Ok(value_mapping(entity))
}

fn value_mapping(entity: Entity) -> ValueMapping {
    IndexMap::from([
        (Name::new("id"), Value::from(entity.id)),
        (Name::new("name"), Value::from(entity.name)),
        (Name::new("partitionId"), Value::from(entity.partition_id)),
        (Name::new("keys"), Value::from(entity.keys.unwrap_or_default())),
        (Name::new("transactionHash"), Value::from(entity.transaction_hash)),
        (
            Name::new("createdAt"),
            Value::from(entity.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        ),
    ])
}
