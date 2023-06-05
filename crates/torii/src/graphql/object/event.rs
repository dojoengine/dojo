use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, Result, Sqlite};

use super::system_call::system_call_by_id;
use super::{ObjectTrait, TypeMapping, ValueMapping};
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
                (Name::new("createdAt"), ScalarType::DATE_TIME.to_string()),
            ]),
        }
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
                    let event_values = event_by_id(&mut conn, &id).await?;

                    Ok(Some(FieldValue::owned_any(event_values)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        ]
    }

    fn nested_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("systemCall", TypeRef::named_nn("SystemCall"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let event_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;
                let syscall_id = extract::<i64>(event_values, "system_call_id")?;
                let system_call = system_call_by_id(&mut conn, syscall_id).await?;

                Ok(Some(FieldValue::owned_any(system_call)))
            })
        })])
    }
}

async fn event_by_id(conn: &mut PoolConnection<Sqlite>, id: &str) -> Result<ValueMapping> {
    let event: Event =
        sqlx::query_as("SELECT * FROM events WHERE id = $1").bind(id).fetch_one(conn).await?;

    Ok(value_mapping(event))
}

fn value_mapping(event: Event) -> ValueMapping {
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
