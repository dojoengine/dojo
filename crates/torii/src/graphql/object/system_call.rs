use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, Result, Sqlite};

use super::query::{query_all, query_by_id, ID};
use super::system::SystemObject;
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::constants::DEFAULT_LIMIT;
use crate::graphql::types::ScalarType;
use crate::graphql::utils::extract_value::extract;

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemCall {
    pub id: i64,
    pub transaction_hash: String,
    pub data: String,
    pub created_at: DateTime<Utc>,
    pub system_id: String,
}
pub struct SystemCallObject {
    pub field_type_mapping: TypeMapping,
}

impl SystemCallObject {
    pub fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::ID.to_string()),
                (Name::new("transactionHash"), TypeRef::STRING.to_string()),
                (Name::new("data"), TypeRef::STRING.to_string()),
                (Name::new("systemId"), TypeRef::ID.to_string()),
                (Name::new("createdAt"), ScalarType::DateTime.to_string()),
            ]),
        }
    }

    pub fn value_mapping(system_call: SystemCall) -> ValueMapping {
        IndexMap::from([
            (Name::new("id"), Value::from(system_call.id.to_string())),
            (Name::new("transactionHash"), Value::from(system_call.transaction_hash)),
            (Name::new("data"), Value::from(system_call.data)),
            (Name::new("systemId"), Value::from(system_call.system_id)),
            (
                Name::new("createdAt"),
                Value::from(
                    system_call.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            ),
        ])
    }
}

impl ObjectTrait for SystemCallObject {
    fn name(&self) -> &str {
        "systemCall"
    }

    fn type_name(&self) -> &str {
        "SystemCall"
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.i64()?;
                    let system_call = query_by_id(&mut conn, "system_calls", ID::I64(id)).await?;
                    let result = SystemCallObject::value_mapping(system_call);
                    Ok(Some(FieldValue::owned_any(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::INT))),
            Field::new("systemCalls", TypeRef::named_list(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let limit = ctx
                        .args
                        .try_get("limit")
                        .and_then(|limit| limit.u64())
                        .unwrap_or(DEFAULT_LIMIT);

                    let system_calls: Vec<SystemCall> =
                        query_all(&mut conn, "system_calls", limit).await?;
                    let result: Vec<FieldValue<'_>> = system_calls
                        .into_iter()
                        .map(SystemCallObject::value_mapping)
                        .map(FieldValue::owned_any)
                        .collect();

                    Ok(Some(FieldValue::list(result)))
                })
            }),
        ]
    }

    fn nested_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("system", TypeRef::named_nn("System"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let syscall_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;
                let system_id = extract::<String>(syscall_values, "systemId")?;
                let system = query_by_id(&mut conn, "systems", ID::Str(system_id)).await?;
                let result = SystemObject::value_mapping(system);
                Ok(Some(FieldValue::owned_any(result)))
            })
        })])
    }
}

pub async fn system_calls_by_system_id(
    conn: &mut PoolConnection<Sqlite>,
    id: &str,
) -> Result<Vec<ValueMapping>> {
    let system_calls: Vec<SystemCall> =
        sqlx::query_as("SELECT * FROM system_calls WHERE system_id = $1")
            .bind(id)
            .fetch_all(conn)
            .await?;

    Ok(system_calls.into_iter().map(SystemCallObject::value_mapping).collect())
}
