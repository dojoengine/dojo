use std::collections::HashMap;

use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, ResolverContext, TypeRef,
};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Pool, QueryBuilder, Row, Sqlite};

use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::constants::DEFAULT_LIMIT;
use crate::graphql::object::entity::{Entity, EntityObject};
use crate::graphql::object::query::{query_by_id, ID};
use crate::graphql::types::ScalarType;
use crate::graphql::utils::extract_value::extract;

const BOOLEAN_TRUE: i64 = 1;
const ENTITY_ID: &str = "entity_id";

pub type ComponentFilters = HashMap<String, String>;

#[derive(FromRow, Deserialize)]
pub struct ComponentMembers {
    pub component_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub key: bool,
    pub created_at: DateTime<Utc>,
}

pub struct ComponentStateObject {
    pub name: String,
    pub type_name: String,
    pub field_type_mapping: TypeMapping,
}

impl ComponentStateObject {
    pub fn new(name: String, type_name: String, field_type_mapping: TypeMapping) -> Self {
        Self { name, type_name, field_type_mapping }
    }
}

impl ObjectTrait for ComponentStateObject {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn nested_fields(&self) -> Option<Vec<Field>> {
        Some(vec![entity_field()])
    }

    fn resolvers(&self) -> Vec<Field> {
        vec![resolve_many(
            self.name.to_string(),
            self.type_name.to_string(),
            self.field_type_mapping.clone(),
        )]
    }
}

fn entity_field() -> Field {
    Field::new("entity", TypeRef::named("Entity"), |ctx| {
        FieldFuture::new(async move {
            let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
            let mapping = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;
            let id = extract::<String>(mapping, ENTITY_ID)?;
            let entity: Entity = query_by_id(&mut conn, "entities", ID::Str(id)).await?;
            let result = EntityObject::value_mapping(entity);

            Ok(Some(FieldValue::owned_any(result)))
        })
    })
}

fn resolve_many(name: String, type_name: String, field_type_mapping: TypeMapping) -> Field {
    let ftm_clone = field_type_mapping.clone();

    let field =
        Field::new(format!("{}Components", &name), TypeRef::named_list(type_name), move |ctx| {
            // FIX: field_type_mapping and name needs to be passed down to the doubly
            // nested async closures, thus the cloning. could handle this better
            let field_type_mapping = field_type_mapping.clone();
            let name = name.clone();

            FieldFuture::new(async move {
                // parse optional input query params
                let (filters, limit) = parse_inputs(&ctx, &field_type_mapping)?;

                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let state_values =
                    component_states_query(&mut conn, &name, &filters, limit, &field_type_mapping)
                        .await?;

                let result: Vec<FieldValue<'_>> =
                    state_values.into_iter().map(FieldValue::owned_any).collect();

                Ok(Some(FieldValue::list(result)))
            })
        });

    add_arguments(field, ftm_clone)
}

fn add_arguments(field: Field, field_type_mapping: TypeMapping) -> Field {
    field_type_mapping
        .into_iter()
        .fold(field, |field, (name, ty)| {
            // omit entity id as argument
            match name.as_str() {
                ENTITY_ID => field,
                _ => field.argument(InputValue::new(name.as_str(), TypeRef::named(ty))),
            }
        })
        .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT)))
}

fn parse_inputs(
    ctx: &ResolverContext<'_>,
    field_type_mapping: &TypeMapping,
) -> async_graphql::Result<(ComponentFilters, u64), async_graphql::Error> {
    let mut filters: ComponentFilters = ComponentFilters::new();

    // parse inputs based on field type mapping
    for (name, ty) in field_type_mapping.iter() {
        let input_option = ctx.args.try_get(name.as_str());

        if let Ok(input) = input_option {
            let input_str = match ScalarType::from_str(ty)? {
                scalar if scalar.is_numeric_type() => input.u64()?.to_string(),
                _ => input.string()?.to_string(),
            };

            filters.insert(name.to_string(), input_str);
        }
    }

    let limit = ctx.args.try_get("limit").and_then(|limit| limit.u64()).unwrap_or(DEFAULT_LIMIT);

    Ok((filters, limit))
}

pub async fn component_state_by_entity_id(
    conn: &mut PoolConnection<Sqlite>,
    name: &str,
    id: &str,
    fields: &TypeMapping,
) -> sqlx::Result<ValueMapping> {
    let table_name = format!("external_{}", name);
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM ");
    builder.push(table_name).push(" WHERE entity_id = ").push_bind(id);
    let row = builder.build().fetch_one(conn).await?;
    value_mapping_from_row(&row, fields)
}

pub async fn component_states_query(
    conn: &mut PoolConnection<Sqlite>,
    name: &str,
    filters: &ComponentFilters,
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
    }
    builder.push(" ORDER BY created_at DESC LIMIT ").push_bind(limit.to_string());

    let component_states = builder.build().fetch_all(conn).await?;
    component_states.iter().map(|row| value_mapping_from_row(row, fields)).collect()
}

fn value_mapping_from_row(row: &SqliteRow, fields: &TypeMapping) -> sqlx::Result<ValueMapping> {
    fields
        .iter()
        .map(|(name, ty)| {
            // handle entity_id separately since it's not a component member
            match name.as_str() {
                ENTITY_ID => Ok((Name::new(name), fetch_string(row, name)?)),
                _ => Ok((Name::new(name), fetch_value(row, name, ty)?)),
            }
        })
        .collect::<sqlx::Result<ValueMapping>>()
}

fn fetch_value(row: &SqliteRow, field_name: &str, field_type: &str) -> sqlx::Result<Value> {
    let column_name = format!("external_{}", field_name);

    match ScalarType::from_str(field_type) {
        Ok(ScalarType::Bool) => fetch_boolean(row, &column_name),
        Ok(ty) if ty.is_numeric_type() => fetch_numeric(row, &column_name),
        Ok(_) => fetch_string(row, &column_name),
        _ => Err(sqlx::Error::TypeNotFound { type_name: field_type.to_string() }),
    }
}

fn fetch_string(row: &SqliteRow, column_name: &str) -> sqlx::Result<Value> {
    row.try_get::<String, &str>(column_name).map(Value::from)
}

fn fetch_numeric(row: &SqliteRow, column_name: &str) -> sqlx::Result<Value> {
    row.try_get::<i64, &str>(column_name).map(Value::from)
}

fn fetch_boolean(row: &SqliteRow, column_name: &str) -> sqlx::Result<Value> {
    let result = row.try_get::<i64, &str>(column_name);
    Ok(Value::from(matches!(result?, BOOLEAN_TRUE)))
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
                    key,
                    created_at
                FROM component_members WHERE component_id = ?
            "#,
    )
    .bind(component_id)
    .fetch_all(conn)
    .await?;

    // field type mapping is 1:1 to component members, but entity_id
    // is not a member so we need to add it manually
    let mut field_type_mapping = TypeMapping::new();
    field_type_mapping.insert(Name::new(ENTITY_ID), TypeRef::ID.to_string());

    for member in component_members {
        field_type_mapping.insert(Name::new(member.name), member.ty);
    }

    Ok(field_type_mapping)
}
