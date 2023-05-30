use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, Result, Sqlite};

// use super::system::System;
use super::{ObjectTraitInstance, ObjectTraitStatic, TypeMapping, ValueMapping};

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemCall {
    pub id: i64,
    pub transaction_hash: String,
    pub data: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_deserializing)]
    pub system_id: String,
}
pub struct SystemCallObject {
    pub field_type_mapping: TypeMapping,
}

impl ObjectTraitStatic for SystemCallObject {
    fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), "ID"),
                (Name::new("transactionHash"), "String"),
                (Name::new("data"), "String"),
                (Name::new("createdAt"), "DateTime"),
            ]),
        }
    }

    fn from(field_type_mapping: TypeMapping) -> Self {
        Self { field_type_mapping }
    }
}

impl ObjectTraitInstance for SystemCallObject {
    fn name(&self) -> &str {
        "systemCall"
    }

    fn type_name(&self) -> &str {
        "SystemCall"
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn field_resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.i64()?;
                    let system_call = system_call_by_id(&mut conn, id).await?;
                    Ok(Some(FieldValue::owned_any(system_call)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::INT))),
        ]
    }
}

pub async fn system_call_by_id(conn: &mut PoolConnection<Sqlite>, id: i64) -> Result<ValueMapping> {
    let system_call = sqlx::query_as!(
        SystemCall,
        r#"
            SELECT
                id,
                data,
                transaction_hash,
                system_id,
                created_at as "created_at: _"
            FROM system_calls WHERE id = $1
        "#,
        id
    )
    .fetch_one(conn)
    .await?;

    Ok(value_mapping(system_call))
}

fn value_mapping(system_call: SystemCall) -> ValueMapping {
    IndexMap::from([
        (Name::new("id"), Value::from(system_call.id.to_string())),
        (Name::new("transactionHash"), Value::from(system_call.transaction_hash)),
        (Name::new("data"), Value::from(system_call.data)),
        (
            Name::new("createdAt"),
            Value::from(
                system_call
                    .created_at
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
        ),
    ])
}
