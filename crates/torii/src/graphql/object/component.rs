use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::query::{query_all, query_by_id, ID};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::constants::DEFAULT_LIMIT;
use crate::graphql::types::ScalarType;
use crate::graphql::utils::remove_quotes;

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub id: String,
    pub name: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

pub struct ComponentObject {
    pub field_type_mapping: TypeMapping,
}

impl ComponentObject {
    // Not used currently, eventually used for component metadata
    pub fn _new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::ID.to_string()),
                (Name::new("name"), TypeRef::STRING.to_string()),
                (Name::new("classHash"), ScalarType::Felt252.to_string()),
                (Name::new("transactionHash"), ScalarType::Felt252.to_string()),
                (Name::new("createdAt"), ScalarType::DateTime.to_string()),
            ]),
        }
    }

    pub fn value_mapping(component: Component) -> ValueMapping {
        IndexMap::from([
            (Name::new("id"), Value::from(component.id)),
            (Name::new("name"), Value::from(component.name)),
            (Name::new("classHash"), Value::from(component.class_hash)),
            (Name::new("transactionHash"), Value::from(component.transaction_hash)),
            (
                Name::new("createdAt"),
                Value::from(
                    component.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            ),
        ])
    }
}

impl ObjectTrait for ComponentObject {
    fn name(&self) -> &str {
        "component"
    }

    fn type_name(&self) -> &str {
        "Component"
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
                    let component = query_by_id(&mut conn, "components", ID::Str(id)).await?;
                    let result = ComponentObject::value_mapping(component);
                    Ok(Some(FieldValue::owned_any(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
            Field::new("components", TypeRef::named_list(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let limit = ctx
                        .args
                        .try_get("limit")
                        .and_then(|limit| limit.u64())
                        .unwrap_or(DEFAULT_LIMIT);

                    let components: Vec<Component> =
                        query_all(&mut conn, "components", limit).await?;
                    let result: Vec<FieldValue<'_>> = components
                        .into_iter()
                        .map(ComponentObject::value_mapping)
                        .map(FieldValue::owned_any)
                        .collect();

                    Ok(Some(FieldValue::list(result)))
                })
            })
            .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT))),
        ]
    }
}
