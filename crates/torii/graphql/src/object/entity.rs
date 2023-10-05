use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use async_graphql::{Name, Value};
use indexmap::IndexMap;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Result, Sqlite};
use tokio_stream::StreamExt;
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Entity;

use super::connection::{
    connection_arguments, connection_output, decode_cursor, parse_connection_arguments,
    ConnectionArguments,
};
use super::model_data::model_data_by_id_query;
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::constants::DEFAULT_LIMIT;
use crate::query::{query_by_id, type_mapping_query};
use crate::types::{GraphqlType, TypeData};
use crate::utils::csv_to_vec;
use crate::utils::extract_value::extract;

pub struct EntityObject {
    pub type_mapping: TypeMapping,
}

impl Default for EntityObject {
    fn default() -> Self {
        Self {
            type_mapping: IndexMap::from([
                (Name::new("id"), TypeData::Simple(TypeRef::named(TypeRef::ID))),
                (Name::new("keys"), TypeData::Simple(TypeRef::named_list(TypeRef::STRING))),
                (Name::new("modelNames"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
                (Name::new("eventId"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
                (
                    Name::new("createdAt"),
                    TypeData::Simple(TypeRef::named(GraphqlType::DateTime.to_string())),
                ),
                (
                    Name::new("updatedAt"),
                    TypeData::Simple(TypeRef::named(GraphqlType::DateTime.to_string())),
                ),
            ]),
        }
    }
}

impl EntityObject {
    pub fn value_mapping(entity: Entity) -> ValueMapping {
        let keys: Vec<&str> = entity.keys.split('/').filter(|&k| !k.is_empty()).collect();
        IndexMap::from([
            (Name::new("id"), Value::from(entity.id)),
            (Name::new("keys"), Value::from(keys)),
            (Name::new("modelNames"), Value::from(entity.model_names)),
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

impl ObjectTrait for EntityObject {
    fn name(&self) -> &str {
        "entity"
    }

    fn type_name(&self) -> &str {
        "Entity"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(vec![model_union_field()])
    }

    fn resolve_one(&self) -> Option<Field> {
        Some(
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.string()?.to_string();
                    let entity = query_by_id(&mut conn, "entities", &id).await?;
                    let result = EntityObject::value_mapping(entity);
                    Ok(Some(Value::Object(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        )
    }

    fn resolve_many(&self) -> Option<Field> {
        let mut field = Field::new(
            "entities",
            TypeRef::named(format!("{}Connection", self.type_name())),
            |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let args = parse_connection_arguments(&ctx)?;
                    let keys = ctx.args.try_get("keys").ok().and_then(|keys| {
                        keys.list().ok().map(|key_list| {
                            key_list
                                    .iter()
                                    .map(|val| val.string().unwrap().to_string()) // safe unwrap
                                    .collect()
                        })
                    });

                    let (entities, total_count) = entities_by_sk(&mut conn, keys, args).await?;

                    Ok(Some(Value::Object(connection_output(entities, total_count))))
                })
            },
        )
        .argument(InputValue::new("keys", TypeRef::named_list(TypeRef::STRING)));

        // Add relay connection fields (first, last, before, after)
        field = connection_arguments(field);

        Some(field)
    }

    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        let name = format!("{}Updated", self.name());
        Some(vec![
            SubscriptionField::new(name, TypeRef::named_nn(self.type_name()), |ctx| {
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

fn model_union_field() -> Field {
    Field::new("models", TypeRef::named_list("ModelUnion"), move |ctx| {
        FieldFuture::new(async move {
            match ctx.parent_value.try_to_value()? {
                Value::Object(indexmap) => {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let model_names = csv_to_vec(&extract::<String>(indexmap, "modelNames")?);
                    let entity_id = extract::<String>(indexmap, "id")?;

                    let mut results: Vec<FieldValue<'_>> = Vec::new();
                    for name in model_names {
                        let type_mapping = type_mapping_query(&mut conn, &name).await?;
                        let state =
                            model_data_by_id_query(&mut conn, &name, &entity_id, &type_mapping)
                                .await?;
                        results.push(FieldValue::with_type(FieldValue::owned_any(state), name));
                    }

                    Ok(Some(FieldValue::list(results)))
                }
                _ => Err("incorrect value, requires Value::Object".into()),
            }
        })
    })
}

async fn entities_by_sk(
    conn: &mut PoolConnection<Sqlite>,
    keys: Option<Vec<String>>,
    args: ConnectionArguments,
) -> Result<(Vec<ValueMapping>, i64)> {
    let mut count_query = "SELECT COUNT(*) FROM entities".to_string();
    let mut entities_query = "SELECT * FROM entities".to_string();
    let mut conditions = Vec::new();

    if let Some(keys) = &keys {
        let keys_str = keys.join("/");
        conditions.push(format!("keys LIKE '{}/%'", keys_str));
        count_query.push_str(&format!(" WHERE keys LIKE '{}/%'", keys_str));
    }

    if let Some(after_cursor) = &args.after {
        match decode_cursor(after_cursor.clone()) {
            Ok((created_at, id)) => {
                conditions.push(format!("(created_at, id) < ('{}', '{}')", created_at, id));
            }
            Err(_) => return Err(sqlx::Error::Decode("Invalid after cursor format".into())),
        }
    }

    if let Some(before_cursor) = &args.before {
        match decode_cursor(before_cursor.clone()) {
            Ok((created_at, id)) => {
                conditions.push(format!("(created_at, id) > ('{}', '{}')", created_at, id));
            }
            Err(_) => return Err(sqlx::Error::Decode("Invalid before cursor format".into())),
        }
    }

    if !conditions.is_empty() {
        let condition_string = conditions.join(" AND ");
        entities_query.push_str(&format!(" WHERE {}", condition_string));
    }

    let limit = args.first.or(args.last).unwrap_or(DEFAULT_LIMIT);
    let order = if args.first.is_some() { "DESC" } else { "ASC" };

    entities_query
        .push_str(&format!(" ORDER BY created_at {}, id {} LIMIT {}", order, order, limit));

    let entities: Vec<Entity> = sqlx::query_as(&entities_query).fetch_all(conn.as_mut()).await?;
    let total_result: (i64,) = sqlx::query_as(&count_query).fetch_one(conn.as_mut()).await?;

    Ok((entities.into_iter().map(EntityObject::value_mapping).collect(), total_result.0))
}
