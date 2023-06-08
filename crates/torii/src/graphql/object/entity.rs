use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, QueryBuilder, Result, Sqlite};

use super::query::{query_by_id, ID};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::constants::DEFAULT_LIMIT;
use crate::graphql::types::ScalarType;
use crate::graphql::utils::remove_quotes;

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub partition: String,
    pub keys: Option<String>,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

pub struct EntityObject {
    pub field_type_mapping: TypeMapping,
}

impl EntityObject {
    pub fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::ID.to_string()),
                (Name::new("partition"), ScalarType::FELT.to_string()),
                (Name::new("keys"), TypeRef::STRING.to_string()),
                (Name::new("transactionHash"), ScalarType::FELT.to_string()),
                (Name::new("createdAt"), ScalarType::DATE_TIME.to_string()),
            ]),
        }
    }

    pub fn value_mapping(entity: Entity) -> ValueMapping {
        IndexMap::from([
            (Name::new("id"), Value::from(entity.id)),
            (Name::new("partition"), Value::from(entity.partition)),
            (Name::new("keys"), Value::from(entity.keys.unwrap_or_default())),
            (Name::new("transactionHash"), Value::from(entity.transaction_hash)),
            (
                Name::new("createdAt"),
                Value::from(entity.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
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

    fn resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = remove_quotes(ctx.args.try_get("id")?.string()?);
                    let entity = query_by_id(&mut conn, "entities", ID::Str(id)).await?;
                    let result = EntityObject::value_mapping(entity);
                    Ok(Some(FieldValue::owned_any(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
            Field::new("entities", TypeRef::named_nn_list_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let parition = remove_quotes(ctx.args.try_get("partition")?.string()?);

                    // handle optional keys argument
                    let maybe_keys = ctx.args.try_get("keys").ok();
                    let keys_arr = if let Some(keys_val) = maybe_keys {
                        keys_val
                            .list()?
                            .iter()
                            .map(|val| val.string().ok().map(remove_quotes))
                            .collect()
                    } else {
                        None
                    };

                    let limit = ctx
                        .args
                        .try_get("limit")
                        .and_then(|limit| limit.u64())
                        .unwrap_or(DEFAULT_LIMIT);

                    let entities = entities_by_sk(&mut conn, &parition, keys_arr, limit).await?;
                    Ok(Some(FieldValue::list(entities.into_iter().map(FieldValue::owned_any))))
                })
            })
            .argument(InputValue::new("partition", TypeRef::named_nn(ScalarType::FELT)))
            .argument(InputValue::new("keys", TypeRef::named_list(TypeRef::STRING)))
            .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT))),
        ]
    }
}

async fn entities_by_sk(
    conn: &mut PoolConnection<Sqlite>,
    partition: &str,
    keys: Option<Vec<String>>,
    limit: u64,
) -> Result<Vec<ValueMapping>> {
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM entities");
    builder.push(" WHERE partition = ").push_bind(partition);

    if let Some(keys) = keys {
        let keys_str = format!("{}%", keys.join("/"));
        builder.push(" AND keys LIKE ").push_bind(keys_str);
    }

    builder.push(" ORDER BY created_at DESC LIMIT ").push(limit);

    let entities: Vec<Entity> = builder.build_query_as().fetch_all(conn).await?;
    Ok(entities.into_iter().map(EntityObject::value_mapping).collect())
}
