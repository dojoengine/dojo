use async_graphql::dynamic::{Field, FieldFuture, Object, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::FromRow;

use super::ObjectTrait;

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityState {
    pub entity_id: String,
    pub component_id: String,
    pub data: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ObjectTrait for EntityState {
    fn object() -> Object {
        Object::new("EntityState")
            .description("")
            .field(Field::new("entityId", TypeRef::named_nn(TypeRef::ID), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<EntityState>()?.entity_id.clone(),
                    )))
                })
            }))
            .field(Field::new("componentId", TypeRef::named_nn(TypeRef::ID), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<EntityState>()?.component_id.clone(),
                    )))
                })
            }))
            .field(Field::new("data", TypeRef::named(TypeRef::STRING), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value.try_downcast_ref::<EntityState>()?.data.clone(),
                    )))
                })
            }))
            .field(Field::new("createdAt", TypeRef::named_nn("DateTime"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value
                            .try_downcast_ref::<EntityState>()?
                            .created_at
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    )))
                })
            }))
            .field(Field::new("updatedAt", TypeRef::named_nn("DateTime"), |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(Value::from(
                        ctx.parent_value
                            .try_downcast_ref::<EntityState>()?
                            .created_at
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    )))
                })
            }))
    }
}
