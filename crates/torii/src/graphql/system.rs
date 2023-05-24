use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, Object, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::system_call::SystemCall;
use super::ObjectTrait;

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

impl ObjectTrait for System {
    fn object() -> Object {
        Object::new("System")
            .description("")
            .field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(ctx.parent_value.try_downcast_ref::<System>()?.id.clone())))
                })
            }))
            .field(Field::new("name", TypeRef::named_nn(TypeRef::STRING), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<System>()?.name.clone(),
                    )))
                })
            }))
            .field(Field::new("address", TypeRef::named_nn("Address"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<System>()?.address.clone(),
                    )))
                })
            }))
            .field(Field::new("classHash", TypeRef::named("Address"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<System>()?.class_hash.clone(),
                    )))
                })
            }))
            .field(Field::new("transactionHash", TypeRef::named_nn("FieldElement"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<System>()?.transaction_hash.clone(),
                    )))
                })
            }))
            .field(Field::new("createdAt", TypeRef::named_nn("DateTime"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value
                            .try_downcast_ref::<System>()?
                            .created_at
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    )))
                })
            }))
            .field(Field::new("systemCalls", TypeRef::named_list("SystemCall"), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = &ctx.parent_value.try_downcast_ref::<System>()?.id;

                    let result: Vec<SystemCall> =
                        sqlx::query_as("SELECT * FROM system_calls WHERE system_id = ?")
                            .bind(id)
                            .fetch_all(&mut conn)
                            .await?;

                    Ok(Some(FieldValue::list(result.into_iter().map(FieldValue::owned_any))))
                })
            }))
    }

    fn resolvers() -> Vec<Field> {
        let system_resolver = Field::new("system", TypeRef::named_nn("System"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let arg_id = ctx.args.get("id").expect("id not found");
                let id = arg_id.string()?;

                let result: System = sqlx::query_as("SELECT * FROM systems WHERE id = ?")
                    .bind(id)
                    .fetch_one(&mut conn)
                    .await?;

                Ok(Some(FieldValue::owned_any(result)))
            })
        })
        .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID)));

        // TODO: resolve system

        vec![system_resolver]
    }
}
