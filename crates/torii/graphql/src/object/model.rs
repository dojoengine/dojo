use async_graphql::dynamic::{
    Field, FieldFuture, InputValue, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use async_graphql::{Name, Value};
use indexmap::IndexMap;
use sqlx::{Pool, Sqlite};
use tokio_stream::StreamExt;
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Model;

use super::connection::connection_output;
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::constants::DEFAULT_LIMIT;
use crate::query::{query_all, query_by_id, query_total_count, ID};
use crate::types::ScalarType;

pub struct ModelObject {
    pub type_mapping: TypeMapping,
}

impl Default for ModelObject {
    // Eventually used for model metadata
    fn default() -> Self {
        Self {
            type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::named(TypeRef::ID)),
                (Name::new("name"), TypeRef::named(TypeRef::STRING)),
                (Name::new("classHash"), TypeRef::named(ScalarType::Felt252.to_string())),
                (Name::new("transactionHash"), TypeRef::named(ScalarType::Felt252.to_string())),
                (Name::new("createdAt"), TypeRef::named(ScalarType::DateTime.to_string())),
            ]),
        }
    }
}

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
    fn name(&self) -> &str {
        "model"
    }

    fn type_name(&self) -> &str {
        "Model"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }

    fn resolve_one(&self) -> Option<Field> {
        Some(
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.string()?.to_string();
                    let model = query_by_id(&mut conn, "models", ID::Str(id)).await?;
                    let result = ModelObject::value_mapping(model);
                    Ok(Some(Value::Object(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        )
    }

    fn resolve_many(&self) -> Option<Field> {
        Some(Field::new(
            "models",
            TypeRef::named(format!("{}Connection", self.type_name())),
            |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let total_count = query_total_count(&mut conn, "models", &Vec::new()).await?;
                    let data: Vec<Model> = query_all(&mut conn, "models", DEFAULT_LIMIT).await?;
                    let models: Vec<ValueMapping> =
                        data.into_iter().map(ModelObject::value_mapping).collect();

                    Ok(Some(Value::Object(connection_output(models, total_count))))
                })
            },
        ))
    }

    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        let name = format!("{}Registered", self.name());
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
