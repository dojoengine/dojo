use async_graphql::dynamic::indexmap::IndexMap;
use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use async_graphql::{Name, Value};
use async_recursion::async_recursion;
use dojo_types::naming::get_tag;
use dojo_types::schema::Ty;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};
use tokio_stream::StreamExt;
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Entity;

use super::inputs::keys_input::keys_argument;
use super::{BasicObject, ResolvableObject, TypeMapping, ValueMapping};
use crate::constants::{
    DATETIME_FORMAT, ENTITY_ID_COLUMN, ENTITY_NAMES, ENTITY_TABLE, ENTITY_TYPE_NAME,
    EVENT_ID_COLUMN, ID_COLUMN,
};
use crate::mapping::ENTITY_TYPE_MAPPING;
use crate::object::{resolve_many, resolve_one};
use crate::query::{build_type_mapping, value_mapping_from_row};
use crate::types::TypeData;
use crate::utils;
#[derive(Debug)]
pub struct EntityObject;

impl BasicObject for EntityObject {
    fn name(&self) -> (&str, &str) {
        ENTITY_NAMES
    }

    fn type_name(&self) -> &str {
        ENTITY_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &ENTITY_TYPE_MAPPING
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(vec![model_union_field()])
    }
}

impl ResolvableObject for EntityObject {
    fn resolvers(&self) -> Vec<Field> {
        let resolve_one = resolve_one(
            ENTITY_TABLE,
            ID_COLUMN,
            self.name().0,
            self.type_name(),
            self.type_mapping(),
        );

        let mut resolve_many = resolve_many(
            ENTITY_TABLE,
            EVENT_ID_COLUMN,
            self.name().1,
            self.type_name(),
            self.type_mapping(),
        );
        resolve_many = keys_argument(resolve_many);

        vec![resolve_one, resolve_many]
    }

    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        Some(vec![SubscriptionField::new(
            "entityUpdated",
            TypeRef::named_nn(self.type_name()),
            |ctx| {
                SubscriptionFieldFuture::new(async move {
                    let id = match ctx.args.get("id") {
                        Some(id) => Some(id.string()?.to_string()),
                        None => None,
                    };
                    // if id is None, then subscribe to all entities
                    // if id is Some, then subscribe to only the entity with that id
                    Ok(SimpleBroker::<Entity>::subscribe().filter_map(move |entity: Entity| {
                        if id.is_none() || id == Some(entity.id.clone()) {
                            Some(Ok(Value::Object(EntityObject::value_mapping(entity))))
                        } else {
                            // id != entity.id , then don't send anything, still listening
                            None
                        }
                    }))
                })
            },
        )
        .argument(InputValue::new("id", TypeRef::named(TypeRef::ID)))])
    }
}

impl EntityObject {
    pub fn value_mapping(entity: Entity) -> ValueMapping {
        let keys: Vec<&str> = entity.keys.split('/').filter(|&k| !k.is_empty()).collect();
        IndexMap::from([
            (Name::new("id"), Value::from(entity.id)),
            (Name::new("keys"), Value::from(keys)),
            (Name::new("eventId"), Value::from(entity.event_id)),
            (
                Name::new("createdAt"),
                Value::from(entity.created_at.format(DATETIME_FORMAT).to_string()),
            ),
            (
                Name::new("updatedAt"),
                Value::from(entity.updated_at.format(DATETIME_FORMAT).to_string()),
            ),
            (
                Name::new("executedAt"),
                Value::from(entity.executed_at.format(DATETIME_FORMAT).to_string()),
            ),
        ])
    }
}

fn model_union_field() -> Field {
    Field::new("models", TypeRef::named_list("ModelUnion"), move |ctx| {
        FieldFuture::new(async move {
            match ctx.parent_value.try_to_value()? {
                Value::Object(indexmap) => {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;

                    let entity_id = utils::extract::<String>(indexmap, "id")?;
                    // fetch name from the models table
                    // using the model id (hashed model name)
                    let model_ids: Vec<(String, String, String)> = sqlx::query_as(
                        "SELECT namespace, name, schema
                        FROM models
                        WHERE id IN (    
                            SELECT model_id
                            FROM entity_model
                            WHERE entity_id = ?
                        )",
                    )
                    .bind(&entity_id)
                    .fetch_all(&mut *conn)
                    .await?;

                    let mut results: Vec<FieldValue<'_>> = Vec::new();
                    for (namespace, name, schema) in model_ids {
                        let schema: Ty = serde_json::from_str(&schema).map_err(|e| {
                            anyhow::anyhow!(format!("Failed to parse model schema: {e}"))
                        })?;
                        let type_mapping = build_type_mapping(&namespace, &schema);

                        // Get the table name
                        let table_name = get_tag(&namespace, &name);

                        // Fetch the row data
                        let query = format!("SELECT * FROM [{}] WHERE internal_entity_id = ?", table_name);
                        let row =
                            sqlx::query(&query).bind(&entity_id).fetch_one(&mut *conn).await?;

                        // Use value_mapping_from_row to handle nested structures
                        let data = value_mapping_from_row(&row, &type_mapping, true)?;

                        results.push(FieldValue::with_type(
                            FieldValue::owned_any(data),
                            utils::type_name_from_names(&namespace, &name),
                        ))
                    }

                    Ok(Some(FieldValue::list(results)))
                }
                _ => Err("incorrect value, requires Value::Object".into()),
            }
        })
    })
}
