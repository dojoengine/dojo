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
    pub field_type_mapping: TypeMapping,
}

impl EntityObject {
    pub fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::ID.to_string()),
                (Name::new("keys"), TypeRef::STRING.to_string()),
                (Name::new("componentNames"), TypeRef::STRING.to_string()),
                (Name::new("createdAt"), ScalarType::DateTime.to_string()),
                (Name::new("updatedAt"), ScalarType::DateTime.to_string()),
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

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn nested_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("components", TypeRef::named_list("ComponentUnion"), move |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let entity = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;

                let components = csv_to_vec(&extract::<String>(entity, "componentNames")?);
                let id = extract::<String>(entity, "id")?;

                let mut results: Vec<FieldValue<'_>> = Vec::new();
                for component_name in components {
                    let table_name = component_name.to_lowercase();
                    let field_type_mapping = type_mapping_from(&mut conn, &table_name).await?;
                    let state = component_state_by_entity_id(
                        &mut conn,
                        &table_name,
                        &id,
                        &field_type_mapping,
                    )
                    .await?;
                    results
                        .push(FieldValue::with_type(FieldValue::owned_any(state), component_name));
                }

                Ok(Some(FieldValue::list(results)))
            })
        })])
    }

    fn resolvers(&self) -> Vec<Field> {
        vec![
            resolve_one(self.name(), self.type_name()), // one
            resolve_many("entities", self.type_name()), // many
        ]
    }
}

fn resolve_one(name: &str, type_name: &str) -> Field {
    Field::new(name, TypeRef::named_nn(type_name), |ctx| {
        FieldFuture::new(async move {
            let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
            let id = ctx.args.try_get("id")?.string()?.to_string();
            let entity = query_by_id(&mut conn, "entities", ID::Str(id)).await?;
            let result = EntityObject::value_mapping(entity);
            Ok(Some(FieldValue::owned_any(result)))
        })
    })
    .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID)))
}

fn resolve_many(name: &str, type_name: &str) -> Field {
    Field::new(name, TypeRef::named_list(type_name), |ctx| {
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

            let limit =
                ctx.args.try_get("limit").and_then(|limit| limit.u64()).unwrap_or(DEFAULT_LIMIT);

            let component_name = ctx.args.try_get("componentName") // Add the component name argument
                .and_then(|name| name.string().map(|s| s.to_string())).ok();

            let entities = entities_by_sk(&mut conn, keys, component_name, limit).await?;
            Ok(Some(FieldValue::list(entities.into_iter().map(FieldValue::owned_any))))
        })
    })
    .argument(InputValue::new("keys", TypeRef::named_nn_list_nn(TypeRef::STRING)))
    .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT)))
    .argument(InputValue::new("componentName", TypeRef::named(TypeRef::STRING)))
}

async fn entities_by_sk(
    conn: &mut PoolConnection<Sqlite>,
    keys: Vec<String>,
    component_name: Option<String>, // Add the filter parameter
    limit: u64,
) -> Result<Vec<ValueMapping>> {
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM entities");
    let keys_str = format!("{},%", keys.join(","));
    builder.push(" WHERE keys LIKE ").push_bind(keys_str);

    if let Some(name) = component_name {
        builder
            .push(" AND (")
            .push("component_names = ")
            .push_bind(name.clone())
            .push(" OR ")
            .push("component_names LIKE ")
            .push_bind(format!("{},%", name))
            .push(" OR ")
            .push("component_names LIKE ")
            .push_bind(format!("%,{}", name))
            .push(" OR ")
            .push("component_names LIKE ")
            .push_bind(format!("%,{},%", name))
            .push(")");
    }

    builder.push(" ORDER BY created_at DESC LIMIT ").push(limit);

    let entities: Vec<Entity> = builder.build_query_as().fetch_all(conn).await?;
    Ok(entities.into_iter().map(EntityObject::value_mapping).collect())
}
