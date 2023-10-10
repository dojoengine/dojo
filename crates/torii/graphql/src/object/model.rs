use async_graphql::dynamic::{
    Field, FieldFuture, InputValue, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use async_graphql::{Name, Value};
use indexmap::IndexMap;
use sqlx::{Pool, Sqlite};
use tokio_stream::StreamExt;
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Model;

use super::connection::{connection_arguments, connection_output, parse_connection_arguments};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::mapping::MODEL_TYPE_MAPPING;
use crate::query::constants::MODEL_TABLE;
use crate::query::data::{count_rows, fetch_multiple_rows, fetch_single_row};
use crate::query::value_mapping_from_row;

pub struct ModelObject;

// TODO: Refactor subscription to not use this
impl ModelObject {
    pub fn value_mapping(model: Model) -> ValueMapping {
        IndexMap::from([
            (Name::new("id"), Value::from(model.id)),
            (Name::new("name"), Value::from(model.name)),
            (Name::new("classHash"), Value::from(model.class_hash)),
            (Name::new("transactionHash"), Value::from(model.transaction_hash)),
            (
                Name::new("createdAt"),
                Value::from(model.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
            ),
        ])
    }
}

impl ObjectTrait for ModelObject {
    fn name(&self) -> (&str, &str) {
        ("model", "models")
    }

    fn type_name(&self) -> &str {
        "Model"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &MODEL_TYPE_MAPPING
    }

    fn resolve_one(&self) -> Option<Field> {
        Some(
            Field::new(self.name().0, TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.string()?.to_string();
                    let data = fetch_single_row(&mut conn, MODEL_TABLE, "id", &id).await?;
                    let model = value_mapping_from_row(&data, &MODEL_TYPE_MAPPING, false)?;

                    Ok(Some(Value::Object(model)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        )
    }

    fn resolve_many(&self) -> Option<Field> {
        let mut field = Field::new(
            self.name().1,
            TypeRef::named(format!("{}Connection", self.type_name())),
            |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let connection = parse_connection_arguments(&ctx)?;
                    let total_count =
                        count_rows(&mut conn, MODEL_TABLE, &None, &Vec::new()).await?;
                    let data = fetch_multiple_rows(
                        &mut conn,
                        MODEL_TABLE,
                        "id",
                        &None,
                        &None,
                        &Vec::new(),
                        &connection,
                    )
                    .await?;
                    let results = connection_output(
                        &data,
                        &MODEL_TYPE_MAPPING,
                        &None,
                        "id",
                        total_count,
                        false,
                    )?;

                    Ok(Some(Value::Object(results)))
                })
            },
        );

        field = connection_arguments(field);

        Some(field)
    }

    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        let name = format!("{}Registered", self.name().0);
        Some(vec![
            SubscriptionField::new(name, TypeRef::named_nn(self.type_name()), |ctx| {
                {
                    SubscriptionFieldFuture::new(async move {
                        let id = match ctx.args.get("id") {
                            Some(id) => Some(id.string()?.to_string()),
                            None => None,
                        };
                        // if id is None, then subscribe to all models
                        // if id is Some, then subscribe to only the model with that id
                        Ok(SimpleBroker::<Model>::subscribe().filter_map(move |model: Model| {
                            if id.is_none() || id == Some(model.id.clone()) {
                                Some(Ok(Value::Object(ModelObject::value_mapping(model))))
                            } else {
                                // id != model.id, so don't send anything, still listening
                                None
                            }
                        }))
                    })
                }
            })
            .argument(InputValue::new("id", TypeRef::named(TypeRef::ID))),
        ])
    }
}
