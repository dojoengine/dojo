use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, Object, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::entity_state::EntityState;
use super::ObjectTrait;

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub id: String,
    pub name: String,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub gql_schema: String,
    pub created_at: DateTime<Utc>,
}

impl ObjectTrait for Component {
    fn object() -> Object {
        Object::new("Component")
            .description("")
            .field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Component>()?.id.clone(),
                    )))
                })
            }))
            .field(Field::new("name", TypeRef::named_nn(TypeRef::STRING), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Component>()?.name.clone(),
                    )))
                })
            }))
            .field(Field::new("address", TypeRef::named_nn("Address"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Component>()?.address.clone(),
                    )))
                })
            }))
            .field(Field::new("classHash", TypeRef::named("Address"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Component>()?.class_hash.clone(),
                    )))
                })
            }))
            .field(Field::new("transactionHash", TypeRef::named_nn("FieldElement"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Component>()?.transaction_hash.clone(),
                    )))
                })
            }))
            .field(Field::new("gqlSchema", TypeRef::named_nn(TypeRef::STRING), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<Component>()?.gql_schema.clone(),
                    )))
                })
            }))
            .field(Field::new("createdAt", TypeRef::named_nn("DateTime"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value
                            .try_downcast_ref::<Component>()?
                            .created_at
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    )))
                })
            }))
            .field(Field::new("entityStates", TypeRef::named_list("EntityState"), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = &ctx.parent_value.try_downcast_ref::<Component>()?.id;

                    let result: Vec<EntityState> =
                        sqlx::query_as("SELECT * FROM entity_states WHERE component_id = ?")
                            .bind(id)
                            .fetch_all(&mut conn)
                            .await?;

                    Ok(Some(FieldValue::list(result.into_iter().map(FieldValue::owned_any))))
                })
            }))
    }

    fn resolvers() -> Vec<Field> {
        let component_resolver = Field::new("component", TypeRef::named_nn("Component"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let arg_id = ctx.args.get("id").expect("id not found");
                let id = arg_id.string()?;

                let result: Component = sqlx::query_as("SELECT * FROM components WHERE id = ?")
                    .bind(id)
                    .fetch_one(&mut conn)
                    .await?;

                Ok(Some(FieldValue::owned_any(result)))
            })
        })
        .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID)));

        // TODO: resolve components

        vec![component_resolver]
    }
}
