use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, Result, Sqlite};

use super::system_call::system_calls_by_system_id;
use super::types::ScalarType;
use super::utils::extract_value::extract;
use super::utils::remove_quotes;
use super::{ObjectTrait, TypeMapping, ValueMapping};

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct System {
    pub id: String,
    pub name: String,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

pub struct SystemObject {
    pub field_type_mapping: TypeMapping,
}

impl SystemObject {
    pub fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::ID.to_string()),
                (Name::new("name"), TypeRef::STRING.to_string()),
                (Name::new("address"), ScalarType::ADDRESS.to_string()),
                (Name::new("classHash"), ScalarType::FELT.to_string()),
                (Name::new("transactionHash"), ScalarType::FELT.to_string()),
                (Name::new("createdAt"), ScalarType::DATE_TIME.to_string()),
            ]),
        }
    }
}

impl ObjectTrait for SystemObject {
    fn name(&self) -> &str {
        "system"
    }

    fn type_name(&self) -> &str {
        "System"
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
                    let system_values = system_by_id(&mut conn, &id).await?;
                    Ok(Some(FieldValue::owned_any(system_values)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        ]
    }

    fn nested_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("systemCalls", TypeRef::named_nn_list_nn("SystemCall"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let system_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;

                let id = extract::<String>(system_values, "id")?;
                let system_calls = system_calls_by_system_id(&mut conn, &id).await?;

                Ok(Some(FieldValue::list(system_calls.into_iter().map(FieldValue::owned_any))))
            })
        })])
    }
}

pub async fn system_by_id(conn: &mut PoolConnection<Sqlite>, id: &str) -> Result<ValueMapping> {
    let system: System =
        sqlx::query_as("SELECT * FROM systems WHERE id = $1").bind(id).fetch_one(conn).await?;

    Ok(value_mapping(system))
}

fn value_mapping(system: System) -> ValueMapping {
    IndexMap::from([
        (Name::new("id"), Value::from(system.id)),
        (Name::new("name"), Value::from(system.name)),
        (Name::new("address"), Value::from(system.address)),
        (Name::new("classHash"), Value::from(system.class_hash)),
        (Name::new("transactionHash"), Value::from(system.transaction_hash)),
        (
            Name::new("createdAt"),
            Value::from(system.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        ),
    ])
}
