use std::collections::HashMap;

use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, ResolverContext, TypeRef,
};
use async_graphql::{Name, Value};
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, QueryBuilder, Row, Sqlite};

use super::component::ComponentMembers;
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::constants::DEFAULT_LIMIT;
use crate::graphql::types::ScalarType;

const BOOLEAN_TRUE: i64 = 1;

pub type StorageFilters = HashMap<String, String>;

pub struct StorageObject {
    pub name: String,
    pub type_name: String,
    pub field_type_mapping: TypeMapping,
}

impl StorageObject {
    pub fn new(name: String, type_name: String, field_type_mapping: TypeMapping) -> Self {
        Self { name, type_name, field_type_mapping }
    }
}

impl ObjectTrait for StorageObject {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn resolvers(&self) -> Vec<Field> {
        vec![
            resolve_one(
                self.name.to_string(),
                self.type_name.to_string(),
                self.field_type_mapping.clone(),
            ),
            resolve_many(
                self.name.to_string(),
                self.type_name.to_string(),
                self.field_type_mapping.clone(),
            ),
        ]
    }
}

fn resolve_one(name: String, type_name: String, field_type_mapping: TypeMapping) -> Field {
    Field::new(name.clone(), TypeRef::named_nn(type_name), move |ctx| {
        // FIX: field_type_mapping and name needs to be passed down to the doubly
        // nested async closures, thus the cloning. could handle this better
        let field_type_mapping = field_type_mapping.clone();
        let name = name.clone();

        FieldFuture::new(async move {
            let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
            let key = ctx.args.try_get("key")?.string()?.to_string();
            let result = storage_by_key(&mut conn, &name, &key, &field_type_mapping).await?;

            Ok(Some(FieldValue::owned_any(result)))
        })
    })
    .argument(InputValue::new("key", TypeRef::named_nn(ScalarType::Felt252.to_string())))
}

fn resolve_many(name: String, type_name: String, field_type_mapping: TypeMapping) -> Field {
    let ftm_clone = field_type_mapping.clone();

    let field = Field::new(format!("{}List", &name), TypeRef::named_list(type_name), move |ctx| {
        let field_type_mapping = field_type_mapping.clone();
        let name = name.clone();

        FieldFuture::new(async move {
            // parse optional input query params
            let (filters, limit) = parse_inputs(&ctx, &field_type_mapping)?;

            let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
            let storage_values =
                storages_query(&mut conn, &name, &filters, limit, &field_type_mapping).await?;

            let result: Vec<FieldValue<'_>> =
                storage_values.into_iter().map(FieldValue::owned_any).collect();

            Ok(Some(FieldValue::list(result)))
        })
    });

    add_arguments(field, ftm_clone)
}

fn add_arguments(field: Field, field_type_mapping: TypeMapping) -> Field {
    field_type_mapping
        .into_iter()
        .fold(field, |field, (name, ty)| {
            field.argument(InputValue::new(name.as_str(), TypeRef::named(ty)))
        })
        .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT)))
}

fn parse_inputs(
    ctx: &ResolverContext<'_>,
    field_type_mapping: &TypeMapping,
) -> async_graphql::Result<(StorageFilters, u64), async_graphql::Error> {
    let mut inputs: StorageFilters = StorageFilters::new();

    for (name, ty) in field_type_mapping.iter() {
        let maybe_input = ctx.args.try_get(name.as_str()).ok();
        if let Some(input) = maybe_input {
            let input_str = if ScalarType::from_str(ty)?.is_numeric_type() {
                input.u64()?.to_string()
            } else {
                input.string()?.to_string()
            };

            inputs.insert(name.to_string(), input_str);
        }
    }

    let limit = ctx.args.try_get("limit").and_then(|limit| limit.u64()).unwrap_or(DEFAULT_LIMIT);

    Ok((inputs, limit))
}

pub async fn storage_by_key(
    conn: &mut PoolConnection<Sqlite>,
    name: &str,
    key: &str,
    fields: &TypeMapping,
) -> sqlx::Result<ValueMapping> {
    let table_name = format!("external_{}", name);
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM ");
    builder.push(table_name).push(" WHERE id = ").push_bind(key);

    let row = builder.build().fetch_one(conn).await?;
    value_mapping_from_row(&row, fields)
}

pub async fn storages_query(
    conn: &mut PoolConnection<Sqlite>,
    name: &str,
    filters: &StorageFilters,
    limit: u64,
    fields: &TypeMapping,
) -> sqlx::Result<Vec<ValueMapping>> {
    let table_name = format!("external_{}", name);
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM ");
    builder.push(table_name);

    if !filters.is_empty() {
        builder.push(" WHERE ");
        let mut separated = builder.separated(" AND ");
        for (name, value) in filters.iter() {
            separated.push(format!("external_{} = '{}'", name, value));
        }
        separated.push_unseparated(" ");
    }
    builder.push("ORDER BY created_at DESC LIMIT ").push_bind(limit.to_string());

    let storages = builder.build().fetch_all(conn).await?;
    storages.iter().map(|row| value_mapping_from_row(row, fields)).collect()
}

fn value_mapping_from_row(row: &SqliteRow, fields: &TypeMapping) -> sqlx::Result<ValueMapping> {
    let mut value_mapping = ValueMapping::new();

    for (field_name, field_type) in fields {
        // Column names are prefixed to avoid conflicts with sqlite keywords
        let column_name = format!("external_{}", field_name);

        let value = match ScalarType::from_str(field_type) {
            Ok(ScalarType::Bool) => {
                // sqlite stores booleans as 0 or 1
                let result = row.try_get::<i64, &str>(&column_name);
                Value::from(matches!(result?, BOOLEAN_TRUE))
            }
            Ok(ty) => {
                if ty.is_numeric_type() {
                    let result = row.try_get::<i64, &str>(&column_name);
                    Value::from(result?)
                } else {
                    let result = row.try_get::<String, &str>(&column_name);
                    Value::from(result?)
                }
            }
            _ => return Err(sqlx::Error::TypeNotFound { type_name: field_type.clone() }),
        };
        value_mapping.insert(Name::new(field_name), value);
    }

    Ok(value_mapping)
}

pub async fn type_mapping_from(
    conn: &mut PoolConnection<Sqlite>,
    component_id: &str,
) -> sqlx::Result<TypeMapping> {
    let component_members: Vec<ComponentMembers> = sqlx::query_as(
        r#"
                SELECT 
                    component_id,
                    name,
                    type AS ty,
                    slot,
                    offset,
                    created_at
                FROM component_members WHERE component_id = ?
            "#,
    )
    .bind(component_id)
    .fetch_all(conn)
    .await?;

    // TODO: check if type exists in scalar types
    let field_type_mapping =
        component_members.iter().fold(TypeMapping::new(), |mut acc, member| {
            acc.insert(Name::new(member.name.clone()), member.ty.clone());
            acc
        });

    Ok(field_type_mapping)
}
