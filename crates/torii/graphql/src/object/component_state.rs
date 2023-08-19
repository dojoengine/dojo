use std::str::FromStr;

use async_graphql::dynamic::{Field, FieldFuture, InputObject, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Pool, QueryBuilder, Row, Sqlite};

use super::connection::{
    connection_arguments, decode_cursor, encode_cursor, parse_connection_arguments,
    ConnectionArguments,
};
use super::input::r#where::{parse_where_argument, where_argument, WhereInputObject};
use super::input::InputObjectTrait;
use super::query::query_total_count;
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::constants::DEFAULT_LIMIT;
use crate::object::entity::{Entity, EntityObject};
use crate::object::filter::{Filter, FilterValue};
use crate::object::query::{query_by_id, ID};
use crate::types::ScalarType;
use crate::utils::extract_value::extract;

const BOOLEAN_TRUE: i64 = 1;

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
    pub type_mapping: TypeMapping,
    pub where_input: WhereInputObject,
}

impl ComponentStateObject {
    pub fn new(name: String, type_name: String, type_mapping: TypeMapping) -> Self {
        let where_input = WhereInputObject::new(type_name.as_str(), &type_mapping);
        Self { name, type_name, type_mapping, where_input }
    }
}

impl ObjectTrait for ComponentStateObject {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    // Type mapping contains all component members and their corresponding type
    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }

    // Associate component to its parent entity
    fn nested_fields(&self) -> Option<Vec<Field>> {
        Some(vec![entity_field()])
    }

    fn input_objects(&self) -> Option<Vec<InputObject>> {
        Some(vec![self.where_input.create()])
    }

    fn resolve_many(&self) -> Option<Field> {
        let name = self.name.clone();
        let type_mapping = self.type_mapping.clone();
        let where_mapping = self.where_input.type_mapping.clone();
        let field_name = format!("{}Components", self.name());
        let field_type = format!("{}Connection", self.type_name());

        let mut field = Field::new(field_name, TypeRef::named(field_type), move |ctx| {
            let type_mapping = type_mapping.clone();
            let where_mapping = where_mapping.clone();
            let name = name.clone();

            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let table_name = format!("external_{}", name);
                let filters = parse_where_argument(&ctx, &where_mapping)?;
                let total_count = query_total_count(&mut conn, &table_name, &filters).await?;
                let connection = parse_connection_arguments(&ctx)?;
                let data =
                    component_states_query(&mut conn, &table_name, &connection, &filters).await?;
                let connection = component_connection(&data, &type_mapping, total_count)?;

                Ok(Some(Value::Object(connection)))
            })
        });

        // Add relay connection fields (first, last, before, after, where)
        field = connection_arguments(field);
        field = where_argument(field, self.type_name());

        Some(field)
    }
}

fn entity_field() -> Field {
    Field::new("entity", TypeRef::named("Entity"), |ctx| {
        FieldFuture::new(async move {
            match ctx.parent_value.try_to_value()? {
                Value::Object(indexmap) => {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = extract::<String>(indexmap, "entity_id")?;
                    let entity: Entity = query_by_id(&mut conn, "entities", ID::Str(id)).await?;
                    let result = EntityObject::value_mapping(entity);

                    Ok(Some(Value::Object(result)))
                }
                _ => Err("incorrect value, requires Value::Object".into()),
            }
        })
    })
}

pub async fn component_state_by_id_query(
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
    table_name: &str,
    connection: &ConnectionArguments,
    filters: &Vec<Filter>,
) -> sqlx::Result<Vec<SqliteRow>> {
    let mut query = format!("SELECT * FROM {}", table_name);
    let mut conditions = Vec::new();

    if let Some(after_cursor) = &connection.after {
        match decode_cursor(after_cursor.clone()) {
            Ok((created_at, id)) => {
                conditions.push(format!("(created_at, entity_id) < ('{}', '{}')", created_at, id));
            }
            Err(_) => return Err(sqlx::Error::Decode("Invalid after cursor format".into())),
        }
    }

    if let Some(before_cursor) = &connection.before {
        match decode_cursor(before_cursor.clone()) {
            Ok((created_at, id)) => {
                conditions.push(format!("(created_at, entity_id) > ('{}', '{}')", created_at, id));
            }
            Err(_) => return Err(sqlx::Error::Decode("Invalid before cursor format".into())),
        }
    }

    for filter in filters {
        let condition = match filter.value {
            FilterValue::Int(i) => format!("{} {} {}", filter.field, filter.comparator, i),
            FilterValue::String(ref s) => format!("{} {} '{}'", filter.field, filter.comparator, s),
        };

        conditions.push(condition);
    }

    if !conditions.is_empty() {
        query.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
    }

    let limit = connection.first.or(connection.last).unwrap_or(DEFAULT_LIMIT);
    let order = if connection.first.is_some() { "DESC" } else { "ASC" };

    query.push_str(&format!(" ORDER BY created_at {}, entity_id {} LIMIT {}", order, order, limit));

    sqlx::query(&query).fetch_all(conn).await
}

// TODO: make `connection_output()` more generic. Currently, `component_connection()` method
// required as we need to explicity add `entity_id` to each edge.
pub fn component_connection(
    data: &[SqliteRow],
    types: &TypeMapping,
    total_count: i64,
) -> sqlx::Result<ValueMapping> {
    let component_edges = data
        .iter()
        .map(|row| {
            // entity_id and created_at used to create cursor
            let entity_id = row.try_get::<String, &str>("entity_id")?;
            let created_at = row.try_get::<String, &str>("created_at")?;
            let cursor = encode_cursor(&created_at, &entity_id);

            // insert entity_id because it needs to be queriable
            let mut value_mapping = value_mapping_from_row(row, types)?;
            value_mapping.insert(Name::new("entity_id"), Value::String(entity_id));

            let mut edge = ValueMapping::new();
            edge.insert(Name::new("node"), Value::Object(value_mapping));
            edge.insert(Name::new("cursor"), Value::String(cursor));

            Ok(Value::Object(edge))
        })
        .collect::<sqlx::Result<Vec<Value>>>();

    Ok(ValueMapping::from([
        (Name::new("totalCount"), Value::from(total_count)),
        (Name::new("edges"), Value::List(component_edges?)),
        // TODO: add pageInfo
    ]))
}

fn value_mapping_from_row(row: &SqliteRow, types: &TypeMapping) -> sqlx::Result<ValueMapping> {
    types
        .iter()
        .map(|(name, ty)| Ok((Name::new(name), fetch_value(row, name, &ty.to_string())?)))
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

pub async fn type_mapping_query(
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
                FROM component_members WHERE key == FALSE AND component_id = ?
            "#,
    )
    .bind(component_id)
    .fetch_all(conn)
    .await?;

    let mut type_mapping = TypeMapping::new();
    for member in component_members {
        type_mapping.insert(Name::new(member.name), TypeRef::named(member.ty));
    }

    Ok(type_mapping)
}
