use std::collections::HashMap;

use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

// use super::system_call::SystemCall;
use super::{FieldTypeMapping, FieldValueMapping, ObjectTrait};

lazy_static! {
    pub static ref SYSTEM_TYPE_MAPPING: FieldTypeMapping = HashMap::from([
        (String::from("id"), String::from("ID")),
        (String::from("name"), String::from("String")),
        (String::from("address"), String::from("Address")),
        (String::from("classHash"), String::from("FieldElement")),
        (String::from("transactionHash"), String::from("FieldElement")),
        (String::from("createdAt"), String::from("DateTime")),
    ]);
}

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
    pub field_type_mappings: FieldTypeMapping,
}

impl ObjectTrait for SystemObject {
    fn new(field_type_mappings: FieldTypeMapping) -> Self {
        Self { field_type_mappings }
    }

    fn name(&self) -> &str {
        "system"
    }

    fn type_name(&self) -> &str {
        "System"
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

                    let system: System = sqlx::query_as("SELECT * FROM systems WHERE id = ?")
                        .bind(id.string()?)
                        .fetch_one(&mut conn)
                        .await?;

                    let result: FieldValueMapping = HashMap::from([
                        (String::from("id"), Value::from(system.id)),
                        (String::from("name"), Value::from(system.name)),
                        (String::from("address"), Value::from(system.address)),
                        (String::from("classHash"), Value::from(system.class_hash)),
                        (String::from("transactionHash"), Value::from(system.transaction_hash)),
                        (
                            String::from("createdAt"),
                            Value::from(
                                system
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
