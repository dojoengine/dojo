use async_graphql::dynamic::{Field, FieldFuture, FieldValue, Object, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::system::System;
use super::ObjectTrait;

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

impl ObjectTrait for SystemCall {
    fn object() -> Object {
        Object::new("SystemCall")
            .description("")
            .field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(ctx.parent_value.try_downcast_ref::<SystemCall>()?.id)))
                })
            }))
            .field(Field::new("data", TypeRef::named_nn(TypeRef::STRING), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<SystemCall>()?.data.clone(),
                    )))
                })
            }))
            .field(Field::new("transactionHash", TypeRef::named_nn("FieldElement"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<SystemCall>()?.transaction_hash.clone(),
                    )))
                })
            }))
            .field(Field::new("createdAt", TypeRef::named_nn("DateTime"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value
                            .try_downcast_ref::<SystemCall>()?
                            .created_at
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    )))
                })
            }))
            .field(Field::new("system", TypeRef::named("System"), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = &ctx.parent_value.try_downcast_ref::<SystemCall>()?.system_id;

                    let result: System = sqlx::query_as("SELECT * FROM systems WHERE id = ?")
                        .bind(id)
                        .fetch_one(&mut conn)
                        .await?;

                    Ok(Some(FieldValue::owned_any(result)))
                })
            }))
    }
}
