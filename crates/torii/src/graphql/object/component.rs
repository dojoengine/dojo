use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef, Union};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::query::{query_all, query_by_id, ID};
use super::storage::{storage_by_column, type_mapping_from_definition, ColumnName};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::constants::DEFAULT_LIMIT;
use crate::graphql::types::ScalarType;
use crate::graphql::utils::extract_value::extract;
use crate::graphql::utils::{format_name, remove_quotes};

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub id: String,
    pub name: String,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub storage_definition: String,
    pub created_at: DateTime<Utc>,
}

pub struct ComponentObject {
    pub field_type_mapping: TypeMapping,
    pub storage_names: Vec<String>,
}

impl ComponentObject {
    // Storage names are passed in on new because
    // it builds the related fields dynamically
    pub fn new(storage_names: Vec<String>) -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::ID.to_string()),
                (Name::new("name"), TypeRef::STRING.to_string()),
                (Name::new("address"), ScalarType::ADDRESS.to_string()),
                (Name::new("classHash"), ScalarType::FELT.to_string()),
                (Name::new("transactionHash"), ScalarType::FELT.to_string()),
                (Name::new("storageDefinition"), TypeRef::STRING.to_string()),
                (Name::new("createdAt"), ScalarType::DATE_TIME.to_string()),
            ]),
            storage_names,
        }
    }

    pub fn value_mapping(component: Component) -> ValueMapping {
        IndexMap::from([
            (Name::new("id"), Value::from(component.id)),
            (Name::new("name"), Value::from(component.name)),
            (Name::new("address"), Value::from(component.address)),
            (Name::new("classHash"), Value::from(component.class_hash)),
            (Name::new("transactionHash"), Value::from(component.transaction_hash)),
            (Name::new("storageDefinition"), Value::from(component.storage_definition)),
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

    fn unions(&self) -> Option<Vec<Union>> {
        Some(vec![self.storage_names.iter().fold(Union::new("Storage"), |union, storage| {
            let (_, type_name) = format_name(storage);
            union.possible_type(type_name)
        })])
    }

    fn nested_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("storage", TypeRef::named("Storage"), move |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let component_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;

                let id = extract::<String>(component_values, "id")?;
                let defintion = extract::<String>(component_values, "storageDefinition")?;
                let type_name = extract::<String>(component_values, "name")?;

                let field_type_mapping = type_mapping_from_definition(&defintion)?;
                let storage_values = storage_by_column(
                    &mut conn,
                    ColumnName::ComponentId,
                    &id,
                    &type_name,
                    &field_type_mapping,
                )
                .await?;

                Ok(Some(FieldValue::with_type(FieldValue::owned_any(storage_values), type_name)))
            })
        })])
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
