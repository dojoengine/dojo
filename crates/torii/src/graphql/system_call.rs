use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

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
                    let id = ctx.args.try_get("id")?;

                    let system_call: SystemCall =
                        sqlx::query_as("SELECT * FROM system_calls WHERE id = ?")
                            .bind(id.i64()?)
                            .fetch_one(&mut conn)
                            .await?;

                    let result: ValueMapping = IndexMap::from([
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
                    ]);

                    Ok(Some(FieldValue::owned_any(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        ]
    }
}
