use async_graphql::dynamic::{Field, FieldFuture, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::connection::connection_output;
use super::system_call::{SystemCall, SystemCallObject};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::constants::DEFAULT_LIMIT;
use crate::query::{query_all, query_by_id, query_total_count, ID};
use crate::types::ScalarType;
use crate::utils::extract_value::extract;

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub keys: String,
    pub data: String,
    pub created_at: DateTime<Utc>,
    pub transaction_hash: String,
}

pub struct EventObject {
    pub type_mapping: TypeMapping,
}

impl Default for EventObject {
    fn default() -> Self {
        Self {
            type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::named(TypeRef::ID)),
                (Name::new("keys"), TypeRef::named(TypeRef::STRING)),
                (Name::new("data"), TypeRef::named(TypeRef::STRING)),
                (Name::new("createdAt"), TypeRef::named(ScalarType::DateTime.to_string())),
                (Name::new("transactionHash"), TypeRef::named(TypeRef::STRING)),
            ]),
        }
    }
}
impl EventObject {
    pub fn value_mapping(event: Event) -> ValueMapping {
        IndexMap::from([
            (Name::new("id"), Value::from(event.id)),
            (Name::new("keys"), Value::from(event.keys)),
            (Name::new("data"), Value::from(event.data)),
            (
                Name::new("createdAt"),
                Value::from(event.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
            ),
            (Name::new("transactionHash"), Value::from(event.transaction_hash)),
        ])
    }
}

impl ObjectTrait for EventObject {
    fn name(&self) -> &str {
        "event"
    }

    fn type_name(&self) -> &str {
        "Event"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }

    fn resolve_one(&self) -> Option<Field> {
        Some(
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.string()?.to_string();
                    let event = query_by_id(&mut conn, "events", ID::Str(id)).await?;
                    let result = EventObject::value_mapping(event);
                    Ok(Some(Value::Object(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        )
    }

    fn resolve_many(&self) -> Option<Field> {
        Some(Field::new(
            "events",
            TypeRef::named(format!("{}Connection", self.type_name())),
            |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let total_count = query_total_count(&mut conn, "events", &Vec::new()).await?;
                    let data: Vec<Event> = query_all(&mut conn, "events", DEFAULT_LIMIT).await?;
                    let events: Vec<ValueMapping> =
                        data.into_iter().map(EventObject::value_mapping).collect();

                    Ok(Some(Value::Object(connection_output(events, total_count))))
                })
            },
        ))
    }

    fn nested_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("systemCall", TypeRef::named_nn("SystemCall"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let event_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;
                let syscall_id = extract::<i64>(event_values, "system_call_id")?;
                let system_call: SystemCall =
                    query_by_id(&mut conn, "system_calls", ID::I64(syscall_id)).await?;
                let result = SystemCallObject::value_mapping(system_call);
                Ok(Some(Value::Object(result)))
            })
        })])
    }
}
