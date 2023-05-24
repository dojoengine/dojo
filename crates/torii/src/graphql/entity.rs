use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, Object, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::entity_state::EntityState;
use super::ObjectTrait;

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub partition_id: String,
    pub keys: Option<String>,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

impl ObjectTrait for Entity {
    fn object() -> Object {
        Object::new("Entity")
            .description(
                "An entity is a collection of components that shares the same storage key.",
            )
            .field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(ctx.parent_value.try_downcast_ref::<Entity>()?.id.clone())))
                })
            }))
            .field(Field::new("name", TypeRef::named_nn(TypeRef::STRING), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Entity>()?.name.clone(),
                    )))
                })
            }))
            .field(Field::new("partitionId", TypeRef::named_nn("FieldElement"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Entity>()?.partition_id.clone(),
                    )))
                })
            }))
            .field(Field::new("keys", TypeRef::named(TypeRef::STRING), |ctx| {
                FieldFuture::new(async move {
                    let entity = ctx.parent_value.try_downcast_ref::<Entity>()?;
                    Ok(entity.keys.clone().map(Value::from))
                })
            }))
            .field(Field::new("transactionHash", TypeRef::named_nn("FieldElement"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Entity>()?.transaction_hash.clone(),
                    )))
                })
            }))
            .field(Field::new("createdAt", TypeRef::named_nn("DateTime"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value
                            .try_downcast_ref::<Entity>()?
                            .created_at
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    )))
                })
            }))
            .field(Field::new("entityStates", TypeRef::named_list("EntityState"), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = &ctx.parent_value.try_downcast_ref::<Entity>()?.id;

                    let result: Vec<EntityState> =
                        sqlx::query_as("SELECT * FROM entity_states WHERE entity_id = ?")
                            .bind(id)
                            .fetch_all(&mut conn)
                            .await?;

                    Ok(Some(FieldValue::list(result.into_iter().map(FieldValue::owned_any))))
                })
            }))
    }

    fn resolvers() -> Vec<Field> {
        let entity_resolver = Field::new("entity", TypeRef::named_nn("Entity"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let arg_id = ctx.args.get("id").expect("id not found");
                let id = arg_id.string()?;

                let result: Entity = sqlx::query_as("SELECT * FROM entities WHERE id = ?")
                    .bind(id)
                    .fetch_one(&mut conn)
                    .await?;

                Ok(Some(FieldValue::owned_any(result)))
            })
        })
        .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID)));

        // TODO: entities resolver

        vec![entity_resolver]
    }
}
