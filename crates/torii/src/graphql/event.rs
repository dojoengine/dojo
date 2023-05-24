use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, Object, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::system_call::SystemCall;
use super::ObjectTrait;

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

impl ObjectTrait for Event {
    fn object() -> Object {
        Object::new("Event")
            .description("")
            .field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(ctx.parent_value.try_downcast_ref::<Event>()?.id.clone())))
                })
            }))
            .field(Field::new("keys", TypeRef::named_nn(TypeRef::STRING), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Event>()?.keys.clone(),
                    )))
                })
            }))
            .field(Field::new("data", TypeRef::named(TypeRef::STRING), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Event>()?.data.clone(),
                    )))
                })
            }))
            .field(Field::new("createdAt", TypeRef::named_nn("DateTime"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value
                            .try_downcast_ref::<Event>()?
                            .created_at
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    )))
                })
            }))
            .field(Field::new("systemCall", TypeRef::named("SystemCall"), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = &ctx.parent_value.try_downcast_ref::<Event>()?.system_call_id;

                    let result: SystemCall =
                        sqlx::query_as("SELECT * FROM system_calls WHERE id = ?")
                            .bind(id)
                            .fetch_one(&mut conn)
                            .await?;

                    Ok(Some(FieldValue::owned_any(result)))
                })
            }))
    }

    fn resolvers() -> Vec<Field> {
        let event_resolver = Field::new("event", TypeRef::named_nn("Event"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let arg_id = ctx.args.get("id").expect("id not found");
                let id = arg_id.string()?;

                let result: Event = sqlx::query_as("SELECT * FROM events WHERE id = ?")
                    .bind(id)
                    .fetch_one(&mut conn)
                    .await?;

                Ok(Some(FieldValue::owned_any(result)))
            })
        })
        .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID)));

        // TODO: resolve events

        vec![event_resolver]
    }
}
