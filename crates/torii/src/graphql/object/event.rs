use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::query::{query_all, query_by_id, ID};
use super::system_call::{SystemCall, SystemCallObject};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::constants::DEFAULT_LIMIT;
use crate::graphql::types::ScalarType;
use crate::graphql::utils::extract_value::extract;
use crate::graphql::utils::remove_quotes;

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub keys: String,
    pub data: String,
    pub created_at: DateTime<Utc>,
    pub system_call_id: i64,
}

pub struct EventObject {
    pub field_type_mapping: TypeMapping,
}

impl EventObject {
    pub fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::ID.to_string()),
                (Name::new("keys"), TypeRef::STRING.to_string()),
                (Name::new("data"), TypeRef::STRING.to_string()),
                (Name::new("systemCallId"), TypeRef::INT.to_string()),
                (Name::new("createdAt"), ScalarType::DateTime.to_string()),
            ]),
        }
    }

    pub fn value_mapping(event: Event) -> ValueMapping {
        IndexMap::from([
            (Name::new("id"), Value::from(event.id)),
            (Name::new("keys"), Value::from(event.keys)),
            (Name::new("data"), Value::from(event.data)),
            (Name::new("systemCallId"), Value::from(event.system_call_id)),
            (
                Name::new("createdAt"),
                Value::from(event.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            ),
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

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = remove_quotes(ctx.args.try_get("id")?.string()?);
                    let event = query_by_id(&mut conn, "events", ID::Str(id)).await?;
                    let result = EventObject::value_mapping(event);
                    Ok(Some(FieldValue::owned_any(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
            Field::new("events", TypeRef::named_list(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let limit = ctx
                        .args
                        .try_get("limit")
                        .and_then(|limit| limit.u64())
                        .unwrap_or(DEFAULT_LIMIT);

                    let events: Vec<Event> = query_all(&mut conn, "events", limit).await?;
                    let result: Vec<FieldValue<'_>> = events
                        .into_iter()
                        .map(EventObject::value_mapping)
                        .map(FieldValue::owned_any)
                        .collect();

                    Ok(Some(FieldValue::list(result)))
                })
            })
            .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT))),
        ]
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
                Ok(Some(FieldValue::owned_any(result)))
            })
        })])
    }
}
