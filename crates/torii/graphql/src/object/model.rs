use async_graphql::dynamic::indexmap::IndexMap;
use async_graphql::dynamic::{
    Enum, Field, FieldFuture, InputObject, InputValue, SubscriptionField, SubscriptionFieldFuture,
    TypeRef,
};
use async_graphql::{Name, Value};
use sqlx::{Pool, Sqlite};
use tokio_stream::StreamExt;
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Model;

use super::connection::{connection_arguments, connection_output, parse_connection_arguments};
use super::inputs::order_input::parse_order_argument;
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::constants::{
    ID_COLUMN, MODEL_NAMES, MODEL_ORDER_FIELD_TYPE_NAME, MODEL_ORDER_TYPE_NAME, MODEL_TABLE,
    MODEL_TYPE_NAME, ORDER_ASC, ORDER_DESC, ORDER_DIR_TYPE_NAME,
};
use crate::mapping::MODEL_TYPE_MAPPING;
use crate::query::data::{count_rows, fetch_multiple_rows};

const ORDER_BY_NAME: &str = "NAME";
const ORDER_BY_HASH: &str = "CLASS_HASH";

pub struct ModelObject;

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

    fn table_name(&self) -> Option<&str> {
        Some(MODEL_TABLE)
    }

    fn input_objects(&self) -> Option<Vec<InputObject>> {
        let order_input = InputObject::new(MODEL_ORDER_TYPE_NAME)
            .field(InputValue::new("direction", TypeRef::named_nn(ORDER_DIR_TYPE_NAME)))
            .field(InputValue::new("field", TypeRef::named_nn(MODEL_ORDER_FIELD_TYPE_NAME)));

        Some(vec![order_input])
    }

    fn enum_objects(&self) -> Option<Vec<Enum>> {
        let direction = Enum::new(ORDER_DIR_TYPE_NAME).item(ORDER_ASC).item(ORDER_DESC);
        let field_order =
            Enum::new(MODEL_ORDER_FIELD_TYPE_NAME).item(ORDER_BY_NAME).item(ORDER_BY_HASH);

        Some(vec![direction, field_order])
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
        field = field.argument(InputValue::new("order", TypeRef::named(MODEL_ORDER_TYPE_NAME)));

        Some(field)
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
