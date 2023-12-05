use async_graphql::dynamic::{
    Field, FieldFuture, InputValue, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use async_graphql::{Name, Result, Value};
use sqlx::{Pool, Sqlite};
use tokio_stream::{Stream, StreamExt};
use torii_core::simple_broker::SimpleBroker;
use torii_core::sql::FELT_DELIMITER;
use torii_core::types::Event;

use super::connection::{connection_arguments, connection_output, parse_connection_arguments};
use super::inputs::keys_input::{keys_argument, parse_keys_argument};
use super::{ObjectTrait, TypeMapping};
use crate::constants::{EVENT_NAMES, EVENT_TABLE, EVENT_TYPE_NAME, ID_COLUMN};
use crate::mapping::EVENT_TYPE_MAPPING;
use crate::query::data::{count_rows, fetch_multiple_rows};
use crate::types::ValueMapping;

pub struct EventObject;

impl ObjectTrait for EventObject {
    fn name(&self) -> (&str, &str) {
        EVENT_NAMES
    }

    fn type_name(&self) -> &str {
        EVENT_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &EVENT_TYPE_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(EVENT_TABLE)
    }

    fn resolve_one(&self) -> Option<Field> {
        None
    }

    fn resolve_many(&self) -> Option<Field> {
        let mut field = Field::new(
            self.name().1,
            TypeRef::named(format!("{}Connection", self.type_name())),
            |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let connection = parse_connection_arguments(&ctx)?;
                    let keys = parse_keys_argument(&ctx)?;
                    let total_count = count_rows(&mut conn, EVENT_TABLE, &keys, &None).await?;
                    let (data, page_info) = fetch_multiple_rows(
                        &mut conn,
                        EVENT_TABLE,
                        ID_COLUMN,
                        &keys,
                        &None,
                        &None,
                        &connection,
                        total_count,
                    )
                    .await?;
                    let results = connection_output(
                        &data,
                        &EVENT_TYPE_MAPPING,
                        &None,
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
        field = keys_argument(field);

        Some(field)
    }

    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        Some(vec![
            SubscriptionField::new("eventEmitted", TypeRef::named_nn(self.type_name()), |ctx| {
                SubscriptionFieldFuture::new(async move {
                    let input_keys = parse_keys_argument(&ctx)?;
                    Ok(EventObject::subscription_stream(input_keys))
                })
            })
            .argument(InputValue::new("keys", TypeRef::named_list(TypeRef::STRING))),
        ])
    }
}

impl EventObject {
    fn value_mapping(event: Event) -> ValueMapping {
        let keys: Vec<&str> = event.keys.split('/').filter(|&k| !k.is_empty()).collect();
        let data: Vec<&str> = event.data.split('/').filter(|&k| !k.is_empty()).collect();
        ValueMapping::from([
            (Name::new("id"), Value::from(event.id)),
            (Name::new("keys"), Value::from(keys)),
            (Name::new("data"), Value::from(data)),
            (Name::new("transactionHash"), Value::from(event.transaction_hash)),
            (
                Name::new("createdAt"),
                Value::from(event.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
            ),
        ])
    }

    fn subscription_stream(input_keys: Option<Vec<String>>) -> impl Stream<Item = Result<Value>> {
        SimpleBroker::<Event>::subscribe().filter_map(move |event| {
            EventObject::match_and_map_event(&input_keys, event)
                .map(|value_mapping| Ok(Value::Object(value_mapping)))
        })
    }

    fn match_and_map_event(input_keys: &Option<Vec<String>>, event: Event) -> Option<ValueMapping> {
        if let Some(ref keys) = input_keys {
            if EventObject::match_keys(keys, &event) {
                return Some(EventObject::value_mapping(event));
            }

            // no match, keep listening
            None
        } else {
            // subscribed to all events
            Some(EventObject::value_mapping(event))
        }
    }

    // Checks if the provided keys match the event's keys, allowing '*' as a wildcard. Returns true
    // if all keys match or if a wildcard is present at the respective position.
    pub fn match_keys(input_keys: &[String], event: &Event) -> bool {
        let event_keys: Vec<&str> =
            event.keys.split(FELT_DELIMITER).filter(|s| !s.is_empty()).collect();

        if input_keys.len() > event_keys.len() {
            return false;
        }

        for (input_key, event_key) in input_keys.iter().zip(event_keys.iter()) {
            if input_key != "*" && input_key != event_key {
                return false;
            }
        }

        true
    }
}
