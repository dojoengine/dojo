use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, QueryBuilder, Result, Sqlite};

use super::component_state::{component_state_by_id_query, type_mapping_query};
use super::connection::{
    connection_input, connection_output, decode_cursor, parse_arguments, ConnectionArguments,
};
use super::query::{query_by_id, ID};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::constants::DEFAULT_LIMIT;
use crate::graphql::types::ScalarType;
use crate::graphql::utils::csv_to_vec;
use crate::graphql::utils::extract_value::extract;

#[derive(FromRow, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
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
                            let type_mapping = type_mapping_query(&mut conn, &table_name).await?;
                            let state = component_state_by_id_query(
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
        let mut field = Field::new(
            "entities",
            TypeRef::named(format!("{}Connection", self.type_name())),
            |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let args = parse_arguments(&ctx)?;
                    let keys_value = ctx.args.try_get("keys")?;
                    let keys = keys_value
                        .list()?
                        .iter()
                        .map(
                            |val| val.string().unwrap().to_string(), // safe unwrap
                        )
                        .collect();

                    let (entities, total_count) = entities_by_sk(&mut conn, keys, args).await?;
                    Ok(Some(Value::Object(connection_output(entities, total_count))))
                })
            },
        )
        .argument(InputValue::new("keys", TypeRef::named_nn_list_nn(TypeRef::STRING)));

        // Add relay connection fields (first, last, before, after)
        field = connection_input(field);

        Some(field)
    }
}

async fn entities_by_sk(
    conn: &mut PoolConnection<Sqlite>,
    keys: Vec<String>,
    args: ConnectionArguments,
) -> Result<(Vec<ValueMapping>, i64)> {
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM entities");
    let keys_str = format!("{},%", keys.join(","));
    builder.push(" WHERE keys LIKE ").push_bind(&keys_str);

    if let Some(after_cursor) = &args.after {
        match decode_cursor(after_cursor.clone()) {
            Ok((created_at, id)) => {
                builder.push(" AND (created_at, id) < (");
                builder.push_bind(created_at).push(",");
                builder.push_bind(id).push(") ");
            }
            Err(_) => return Err(sqlx::Error::Decode("Invalid after cursor format".into())),
        }
    }

    if let Some(before_cursor) = &args.before {
        match decode_cursor(before_cursor.clone()) {
            Ok((created_at, id)) => {
                builder.push(" AND (created_at, id) > (");
                builder.push_bind(created_at).push(",");
                builder.push_bind(id).push(") ");
            }
            Err(_) => return Err(sqlx::Error::Decode("Invalid before cursor format".into())),
        }
    }

    if let Some(first) = args.first {
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push(first);
    } else if let Some(last) = args.last {
        builder.push(" ORDER BY created_at ASC, id ASC LIMIT ");
        builder.push(last);
    } else {
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ").push(DEFAULT_LIMIT);
    }

    let entities: Vec<Entity> = builder.build_query_as().fetch_all(conn.as_mut()).await?;
    let total_result: (i64,) =
        sqlx::query_as(&format!("SELECT COUNT(*) FROM entities WHERE keys LIKE '{}'", keys_str))
            .fetch_one(conn)
            .await?;

    Ok((entities.into_iter().map(EntityObject::value_mapping).collect(), total_result.0))
}
