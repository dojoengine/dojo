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
    pub static ref EVENT_TYPE_MAPPING: FieldTypeMapping = HashMap::from([
        (String::from("id"), String::from("ID")),
        (String::from("keys"), String::from("String")),
        (String::from("data"), String::from("String")),
        (String::from("createdAt"), String::from("DateTime")),
    ]);
}

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub keys: String,
    pub data: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_deserializing)]
    pub system_call_id: i64,
}

pub struct EventObject {
    pub field_type_mappings: FieldTypeMapping,
}

impl ObjectTrait for EventObject {
    fn new(field_type_mappings: FieldTypeMapping) -> Self {
        Self { field_type_mappings }
    }

    fn name(&self) -> &str {
        "event"
    }

    fn type_name(&self) -> &str {
        "Event"
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

                    let event: Event = sqlx::query_as("SELECT * FROM events WHERE id = ?")
                        .bind(id.string()?)
                        .fetch_one(&mut conn)
                        .await?;

                    let result: FieldValueMapping = HashMap::from([
                        (String::from("id"), Value::from(event.id)),
                        (String::from("keys"), Value::from(event.keys)),
                        (String::from("data"), Value::from(event.data)),
                        (
                            String::from("createdAt"),
                            Value::from(
                                event.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
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
