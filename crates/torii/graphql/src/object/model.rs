use async_graphql::dynamic::indexmap::IndexMap;
use async_graphql::dynamic::{Enum,Field,FieldFuture, InputValue, SubscriptionField, SubscriptionFieldFuture, TypeRef};
use async_graphql::{Name, Value};
use tokio_stream::StreamExt;
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Model;
use sqlx::{Pool, Sqlite};

use super::{ObjectTrait, TypeMapping, ValueMapping};
use super::inputs::order_input::{order_argument, parse_order_argument, OrderInputObject};
use crate::constants::{MODEL_NAMES, MODEL_TABLE, MODEL_TYPE_NAME};
use crate::mapping::MODEL_TYPE_MAPPING;
use super::inputs::InputObjectTrait;
use super::connection::{connection_arguments, connection_output, parse_connection_arguments};
use crate::constants::ID_COLUMN;
use crate::query::data::{count_rows, fetch_multiple_rows};

pub struct ModelObject{
    pub name: String,
    pub type_name: String,
    pub order_input: OrderInputObject,
}

impl ModelObject {
    pub fn new(name: String, type_name: String ) -> Self {
        let order_input = OrderInputObject::new(type_name.as_str(), &MODEL_TYPE_MAPPING);
        println!("ModelObject {}", name);
        Self { name, type_name, order_input }
    }
}

impl ObjectTrait for ModelObject {
    fn name(&self) -> (&str, &str) {
        MODEL_NAMES
    }

    fn type_name(&self) -> &str {
        MODEL_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &MODEL_TYPE_MAPPING
    }

    fn enum_objects(&self) -> Option<Vec<Enum>> {
        self.order_input.enum_objects()
    }

    fn table_name(&self) -> Option<&str> {
        Some(MODEL_TABLE)
    }

    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        Some(vec![
            SubscriptionField::new("modelRegistered", TypeRef::named_nn(self.type_name()), |ctx| {
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

    fn resolve_many(&self) -> Option<Field> {
        let type_mapping = self.type_mapping().clone();
        let table_name = self.table_name().unwrap().to_string();

        let mut field = Field::new(
            self.name().1,
            TypeRef::named(format!("{}Connection", self.type_name())),
            move |ctx| {
                let type_mapping = type_mapping.clone();
                let table_name = table_name.to_string();

                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let order = parse_order_argument(&ctx);
                    let connection = parse_connection_arguments(&ctx)?;
                    let total_count = count_rows(&mut conn, &table_name, &None, &None).await?;
                    let (data, page_info) = fetch_multiple_rows(
                        &mut conn,
                        &table_name,
                        ID_COLUMN,
                        &None,
                        &order,
                        &None,
                        &connection,
                        total_count,
                    )
                    .await?;
                    let results = connection_output(
                        &data,
                        &type_mapping,
                        &order,
                        ID_COLUMN,
                        total_count,
                        false,
                        page_info,
                    )?;

                    Ok(Some(Value::Object(results)))
                })
            },
        );

        field = connection_arguments(field);
        println!("Name {}", MODEL_NAMES.1);
        field = order_argument(field, self.type_name());
        Some(field)
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
