use async_graphql::dynamic::{Field, FieldFuture, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, Result, Sqlite};

use super::connection::connection_output;
use super::system::SystemObject;
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::constants::DEFAULT_LIMIT;
use crate::query::{query_all, query_by_id, query_total_count, ID};
use crate::types::ScalarType;
use crate::utils::extract_value::extract;

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
    pub type_mapping: TypeMapping,
}

impl SystemCallObject {
    pub fn new() -> Self {
        Self {
            type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::named(TypeRef::ID)),
                (Name::new("transactionHash"), TypeRef::named(TypeRef::STRING)),
                (Name::new("data"), TypeRef::named(TypeRef::STRING)),
                (Name::new("systemId"), TypeRef::named(TypeRef::ID)),
                (Name::new("createdAt"), TypeRef::named(ScalarType::DateTime.to_string())),
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
                Value::from(system_call.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
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

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }

    fn resolve_one(&self) -> Option<Field> {
        Some(
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.i64()?;
                    let system_call = query_by_id(&mut conn, "system_calls", ID::I64(id)).await?;
                    let result = SystemCallObject::value_mapping(system_call);
                    Ok(Some(Value::Object(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::INT))),
        )
    }

    fn resolve_many(&self) -> Option<Field> {
        Some(Field::new(
            "systemCalls",
            TypeRef::named(format!("{}Connection", self.type_name())),
            |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let total_count =
                        query_total_count(&mut conn, "system_calls", &Vec::new()).await?;
                    let data: Vec<SystemCall> =
                        query_all(&mut conn, "system_calls", DEFAULT_LIMIT).await?;
                    let system_calls: Vec<ValueMapping> =
                        data.into_iter().map(SystemCallObject::value_mapping).collect();

                    Ok(Some(Value::Object(connection_output(system_calls, total_count))))
                })
            },
        ))
    }

    fn nested_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("system", TypeRef::named_nn("System"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let syscall_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;
                let system_id = extract::<String>(syscall_values, "systemId")?;
                let system = query_by_id(&mut conn, "systems", ID::Str(system_id)).await?;
                let result = SystemObject::value_mapping(system);
                Ok(Some(Value::Object(result)))
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
