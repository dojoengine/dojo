use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

// use super::system_call::SystemCall;
use super::{ObjectTraitInstance, ObjectTraitStatic, TypeMapping, ValueMapping};

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
    pub field_type_mapping: TypeMapping,
}

impl ObjectTraitStatic for EventObject {
    fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), "ID"),
                (Name::new("keys"), "String"),
                (Name::new("data"), "String"),
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
                    let id = ctx.args.try_get("id")?;

                    let event: Event = sqlx::query_as("SELECT * FROM events WHERE id = ?")
                        .bind(id.string()?)
                        .fetch_one(&mut conn)
                        .await?;

                    let result: ValueMapping = IndexMap::from([
                        (Name::new("id"), Value::from(event.id)),
                        (Name::new("keys"), Value::from(event.keys)),
                        (Name::new("data"), Value::from(event.data)),
                        (
                            Name::new("createdAt"),
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
