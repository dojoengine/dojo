use std::borrow::Cow;

use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, Result, Sqlite};

use super::system_call::system_call_by_id;
use super::utils::value_accessor::ObjectAccessor;
use super::{ObjectTraitInstance, ObjectTraitStatic, TypeMapping, ValueMapping};

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

impl ObjectTraitStatic for EventObject {
    fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), "ID"),
                (Name::new("keys"), "String"),
                (Name::new("data"), "String"),
                (Name::new("systemCallId"), "Int"),
                (Name::new("createdAt"), "DateTime"),
            ]),
        }
    }

    fn from(field_type_mapping: TypeMapping) -> Self {
        Self { field_type_mapping }
    }
}

impl ObjectTraitInstance for EventObject {
    fn name(&self) -> &str {
        "event"
    }

    fn type_name(&self) -> &str {
        "Event"
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn field_resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.string()?.replace('\"', "");
                    let event_values = event_by_id(&mut conn, &id).await?;

                    Ok(Some(FieldValue::owned_any(event_values)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        ]
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("systemCall", TypeRef::named_nn("SystemCall"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let event_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;

                let syscall_id =
                    ObjectAccessor(Cow::Borrowed(event_values)).try_get("system_call_id")?.i64()?;
                let system_call = system_call_by_id(&mut conn, syscall_id).await?;

                Ok(Some(FieldValue::owned_any(system_call)))
            })
        })])
    }
}

async fn event_by_id(conn: &mut PoolConnection<Sqlite>, id: &str) -> Result<ValueMapping> {
    let event = sqlx::query_as!(
        Event,
        r#"
            SELECT 
                id,
                system_call_id,
                keys,
                data,
                created_at as "created_at: _"
            FROM events 
            WHERE id = $1
        "#,
        id
    )
    .fetch_one(conn)
    .await?;

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
