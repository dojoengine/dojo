use std::collections::HashMap;

use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

// use super::system::System;
use super::{FieldTypeMapping, FieldValueMapping, ObjectTraitInstance, ObjectTraitStatic};

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
    pub field_type_mappings: FieldTypeMapping,
}

impl ObjectTraitStatic for SystemCallObject {
    fn new() -> Self {
        Self {
            field_type_mappings: HashMap::from([
                (String::from("id"), String::from("ID")),
                (String::from("transactionHash"), String::from("String")),
                (String::from("data"), String::from("String")),
                (String::from("createdAt"), String::from("DateTime")),
            ]),
        }
    }

    fn from(field_type_mappings: FieldTypeMapping) -> Self {
        Self { field_type_mappings }
    }
}

impl ObjectTraitInstance for SystemCallObject {
    fn name(&self) -> &str {
        "systemCall"
    }

    fn type_name(&self) -> &str {
        "SystemCall"
    }

    fn field_type_mappings(&self) -> &FieldTypeMapping {
        &self.field_type_mappings
    }

    fn field_resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.get("id").expect("id not found");

                    let system_call: SystemCall =
                        sqlx::query_as("SELECT * FROM system_calls WHERE id = ?")
                            .bind(id.i64()?)
                            .fetch_one(&mut conn)
                            .await?;

                    let result: FieldValueMapping = HashMap::from([
                        (String::from("id"), Value::from(system_call.id.to_string())),
                        (
                            String::from("transactionHash"),
                            Value::from(system_call.transaction_hash),
                        ),
                        (String::from("data"), Value::from(system_call.data)),
                        (
                            String::from("createdAt"),
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
