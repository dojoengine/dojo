use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, QueryBuilder, Result, Sqlite};

use super::component_state::{component_state_by_entity_id, type_mapping_from};
use super::query::{query_by_id, ID};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::constants::DEFAULT_LIMIT;
use crate::graphql::types::ScalarType;
use crate::graphql::utils::csv_to_vec;
use crate::graphql::utils::extract_value::extract;

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub partition: String,
    pub keys: Option<String>,
    pub component_names: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct EntityObject {
    pub type_mapping: TypeMapping,
}

impl EntityObject {
    pub fn new() -> Self {
        Self {
            type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::named(TypeRef::ID)),
                (Name::new("keys"), TypeRef::named(TypeRef::STRING)),
                (Name::new("componentNames"), TypeRef::named(TypeRef::STRING)),
                (Name::new("createdAt"), TypeRef::named(ScalarType::DateTime.to_string())),
                (Name::new("updatedAt"), TypeRef::named(ScalarType::DateTime.to_string())),
            ]),
        }
    }

    pub fn value_mapping(entity: Entity) -> ValueMapping {
        IndexMap::from([
            (Name::new("id"), Value::from(entity.id)),
            (Name::new("keys"), Value::from(entity.keys.unwrap_or_default())),
            (Name::new("componentNames"), Value::from(entity.component_names)),
            (
                Name::new("createdAt"),
                Value::from(entity.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            ),
            (
                Name::new("updatedAt"),
                Value::from(entity.updated_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
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

    fn nested_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("components", TypeRef::named_list("ComponentUnion"), move |ctx| {
            FieldFuture::new(async move {
                match ctx.parent_value.try_to_value()? {
                    Value::Object(indexmap) => {
                        let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                        let components =
                            csv_to_vec(&extract::<String>(indexmap, "componentNames")?);
                        let id = extract::<String>(indexmap, "id")?;

                        let mut results: Vec<FieldValue<'_>> = Vec::new();
                        for component_name in components {
                            let table_name = component_name.to_lowercase();
                            let type_mapping = type_mapping_from(&mut conn, &table_name).await?;
                            let state = component_state_by_entity_id(
                                &mut conn,
                                &table_name,
                                &id,
                                &type_mapping,
                            )
                            .await?;
                            results.push(FieldValue::with_type(
                                FieldValue::owned_any(state),
                                component_name,
                            ));
                        }

                        Ok(Some(FieldValue::list(results)))
                    }
                    _ => Err("incorrect value, requires Value::Object".into()),
                }
            })
        })])
    }

    fn resolve_one(&self) -> Option<Field> {
        Some(
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.string()?.to_string();
                    let entity = query_by_id(&mut conn, "entities", ID::Str(id)).await?;
                    let result = EntityObject::value_mapping(entity);
                    Ok(Some(Value::Object(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        )
    }

    fn resolve_many(&self) -> Option<Field> {
        Some(
            Field::new("entities", TypeRef::named_list(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;

                    let keys_value = ctx.args.try_get("keys")?;
                    let keys = keys_value
                        .list()?
                        .iter()
                        .map(
                            |val| val.string().unwrap().to_string(), // safe unwrap
                        )
                        .collect();

                    let limit = ctx
                        .args
                        .try_get("limit")
                        .and_then(|limit| limit.u64())
                        .unwrap_or(DEFAULT_LIMIT);

                    let entities = entities_by_sk(&mut conn, keys, limit).await?;
                    Ok(Some(FieldValue::list(entities.into_iter().map(FieldValue::owned_any))))
                })
            })
            .argument(InputValue::new("keys", TypeRef::named_nn_list_nn(TypeRef::STRING)))
            .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT))),
        )
    }
}

async fn entities_by_sk(
    conn: &mut PoolConnection<Sqlite>,
    keys: Vec<String>,
    limit: u64,
) -> Result<Vec<ValueMapping>> {
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM entities");
    let keys_str = format!("{},%", keys.join(","));
    builder.push(" WHERE keys LIKE ").push_bind(keys_str);
    builder.push(" ORDER BY created_at DESC LIMIT ").push(limit);

    let entities: Vec<Entity> = builder.build_query_as().fetch_all(conn).await?;
    Ok(entities.into_iter().map(EntityObject::value_mapping).collect())
}
