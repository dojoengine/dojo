use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, Result, Sqlite};

use super::types::ScalarType;
use super::utils::remove_quotes;
use super::{ObjectTrait, TypeMapping, ValueMapping};

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

impl EntityObject {
    pub fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::ID.to_string()),
                (Name::new("name"), TypeRef::STRING.to_string()),
                (Name::new("partitionId"), ScalarType::FELT.to_string()),
                (Name::new("keys"), TypeRef::STRING.to_string()),
                (Name::new("transactionHash"), ScalarType::FELT.to_string()),
                (Name::new("createdAt"), ScalarType::DATE_TIME.to_string()),
            ]),
        }
    }
}

impl ObjectTrait for EntityObject {
    fn name(&self) -> &str {
        "entity"
    }

    fn type_name(&self) -> &str {
        "Entity"
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = remove_quotes(ctx.args.try_get("id")?.string()?);
                    let entity_values = entity_by_id(&mut conn, &id).await?;
                    Ok(Some(FieldValue::owned_any(entity_values)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        ]
    }
}

async fn entity_by_id(conn: &mut PoolConnection<Sqlite>, id: &str) -> Result<ValueMapping> {
    let entity: Entity =
        sqlx::query_as("SELECT * FROM entities WHERE id = $1").bind(id).fetch_one(conn).await?;

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
