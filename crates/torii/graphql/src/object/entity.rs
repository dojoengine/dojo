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
                    let model_ids: Vec<(String, String, String, String)> = sqlx::query_as(
                        "SELECT id, namespace, name, schema
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
                    for (id, namespace, name, schema) in model_ids {
                        let schema: Ty = serde_json::from_str(&schema).map_err(|e| {
                            anyhow::anyhow!(format!("Failed to parse model schema: {e}"))
                        })?;
                        let type_mapping = build_type_mapping(&namespace, &schema);

                        // but the table name for the model data is the unhashed model name
                        let data: ValueMapping = match model_data_recursive_query(
                            &mut conn,
                            ENTITY_ID_COLUMN,
                            vec![get_tag(&namespace, &name)],
                            &entity_id,
                            &[],
                            &type_mapping,
                            false,
                        )
                        .await?
                        {
                            Value::Object(map) => map,
                            _ => unreachable!(),
                        };

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

// TODO: flatten query
#[async_recursion]
pub async fn model_data_recursive_query(
    conn: &mut PoolConnection<Sqlite>,
    entity_id_column: &str,
    path_array: Vec<String>,
    entity_id: &str,
    indexes: &[i64],
    type_mapping: &TypeMapping,
    is_list: bool,
) -> sqlx::Result<Value> {
    // For nested types, we need to remove prefix in path array
    let namespace = format!("{}_", path_array[0]);
    let table_name = &path_array.join("$").replace(&namespace, "");
    let mut query =
        format!("SELECT * FROM [{}] WHERE {entity_id_column} = '{}' ", table_name, entity_id);
    for (column_idx, index) in indexes.iter().enumerate() {
        query.push_str(&format!("AND idx_{} = {} ", column_idx, index));
    }

    let rows = sqlx::query(&query).fetch_all(conn.as_mut()).await?;
    if rows.is_empty() {
        return Ok(Value::List(vec![]));
    }

    let value_mapping: Value;
    let mut nested_value_mappings = Vec::new();

    for (idx, row) in rows.iter().enumerate() {
        let mut nested_value_mapping = value_mapping_from_row(row, type_mapping, true)?;

        for (field_name, type_data) in type_mapping {
            if let TypeData::Nested((_, nested_mapping)) = type_data {
                let mut nested_path = path_array.clone();
                nested_path.push(field_name.to_string());

                let nested_values = model_data_recursive_query(
                    conn,
                    entity_id_column,
                    nested_path,
                    entity_id,
                    &if is_list {
                        let mut indexes = indexes.to_vec();
                        indexes.push(idx as i64);
                        indexes
                    } else {
                        indexes.to_vec()
                    },
                    nested_mapping,
                    false,
                )
                .await?;

                nested_value_mapping.insert(Name::new(field_name), nested_values);
            } else if let TypeData::List(inner) = type_data {
                let mut nested_path = path_array.clone();
                nested_path.push(field_name.to_string());

                let data = match model_data_recursive_query(
                    conn,
                    entity_id_column,
                    nested_path,
                    entity_id,
                    // this might need to be changed to support 2d+ arrays
                    &if is_list {
                        let mut indexes = indexes.to_vec();
                        indexes.push(idx as i64);
                        indexes
                    } else {
                        indexes.to_vec()
                    },
                    &IndexMap::from([(Name::new("data"), *inner.clone())]),
                    true,
                )
                .await?
                {
                    // map our list which uses a data field as a place holder
                    // for all elements to get the elemnt directly
                    Value::List(data) => data
                        .iter()
                        .map(|v| match v {
                            Value::Object(map) => map.get(&Name::new("data")).unwrap().clone(),
                            ty => unreachable!(
                                "Expected Value::Object for list \"data\" field, got {:?}",
                                ty
                            ),
                        })
                        .collect(),
                    Value::Object(map) => map.get(&Name::new("data")).unwrap().clone(),
                    ty => {
                        unreachable!(
                            "Expected Value::List or Value::Object for list, got {:?}",
                            ty
                        );
                    }
                };

                nested_value_mapping.insert(Name::new(field_name), data);
            }
        }

        nested_value_mappings.push(Value::Object(nested_value_mapping));
    }

    if is_list {
        value_mapping = Value::List(nested_value_mappings);
    } else {
        value_mapping = nested_value_mappings.pop().unwrap();
    }

    Ok(value_mapping)
}
