use async_graphql::dynamic::indexmap::IndexMap;
use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use async_graphql::{Name, Value};
use async_recursion::async_recursion;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};
use tokio_stream::StreamExt;
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Entity;

use super::connection::{connection_arguments, connection_output, parse_connection_arguments};
use super::inputs::keys_input::{keys_argument, parse_keys_argument};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::constants::{ENTITY_NAMES, ENTITY_TABLE, ENTITY_TYPE_NAME, EVENT_ID_COLUMN};
use crate::mapping::ENTITY_TYPE_MAPPING;
use crate::query::data::{count_rows, fetch_multiple_rows};
use crate::query::{type_mapping_query, value_mapping_from_row};
use crate::types::TypeData;
use crate::utils::extract;
pub struct EntityObject;

impl ObjectTrait for EntityObject {
    fn name(&self) -> (&str, &str) {
        ENTITY_NAMES
    }

    fn type_name(&self) -> &str {
        ENTITY_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &ENTITY_TYPE_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(ENTITY_TABLE)
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(vec![model_union_field()])
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
                    let total_count = count_rows(&mut conn, ENTITY_TABLE, &keys, &None).await?;
                    let (data, page_info) = fetch_multiple_rows(
                        &mut conn,
                        ENTITY_TABLE,
                        EVENT_ID_COLUMN,
                        &keys,
                        &None,
                        &None,
                        &connection,
                        total_count,
                    )
                    .await?;
                    let results = connection_output(
                        &data,
                        &ENTITY_TYPE_MAPPING,
                        &None,
                        EVENT_ID_COLUMN,
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
            SubscriptionField::new("entityUpdated", TypeRef::named_nn(self.type_name()), |ctx| {
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
            })
            .argument(InputValue::new("id", TypeRef::named(TypeRef::ID))),
        ])
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
                Value::from(entity.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
            ),
            (
                Name::new("updatedAt"),
                Value::from(entity.updated_at.format("%Y-%m-%d %H:%M:%S").to_string()),
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

                    let entity_id = extract::<String>(indexmap, "id")?;
                    let model_ids: Vec<(String,)> =
                        sqlx::query_as("SELECT model_id from entity_model WHERE entity_id = ?")
                            .bind(&entity_id)
                            .fetch_all(&mut *conn)
                            .await?;

                    let mut results: Vec<FieldValue<'_>> = Vec::new();
                    for (name,) in model_ids {
                        let type_mapping = type_mapping_query(&mut conn, &name).await?;

                        let data = model_data_recursive_query(
                            &mut conn,
                            vec![name.clone()],
                            &entity_id,
                            &type_mapping,
                        )
                        .await?;

                        results.push(FieldValue::with_type(FieldValue::owned_any(data), name));
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
    path_array: Vec<String>,
    entity_id: &str,
    type_mapping: &TypeMapping,
) -> sqlx::Result<ValueMapping> {
    // For nested types, we need to remove prefix in path array
    let namespace = format!("{}_", path_array[0]);
    let table_name = &path_array.join("$").replace(&namespace, "");
    let query = format!("SELECT * FROM {} WHERE entity_id = '{}'", table_name, entity_id);
    let row = sqlx::query(&query).fetch_one(conn.as_mut()).await?;
    let mut value_mapping = value_mapping_from_row(&row, type_mapping, true)?;

    for (field_name, type_data) in type_mapping {
        if let TypeData::Nested((_, nested_mapping)) = type_data {
            let mut nested_path = path_array.clone();
            nested_path.push(field_name.to_string());

            let nested_values =
                model_data_recursive_query(conn, nested_path, entity_id, nested_mapping).await?;

            value_mapping.insert(Name::new(field_name), Value::Object(nested_values));
        }
    }

    Ok(value_mapping)
}
