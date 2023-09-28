use std::str::FromStr;

use async_graphql::dynamic::{Enum, Field, FieldFuture, InputObject, Object, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use dojo_types::core::CairoType;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Pool, QueryBuilder, Row, Sqlite};
use torii_core::types::Entity;

use super::connection::{
    connection_arguments, decode_cursor, encode_cursor, parse_connection_arguments,
    ConnectionArguments,
};
use super::inputs::order_input::{order_argument, parse_order_argument, OrderInputObject};
use super::inputs::where_input::{parse_where_argument, where_argument, WhereInputObject};
use super::inputs::InputObjectTrait;
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::constants::DEFAULT_LIMIT;
use crate::object::entity::EntityObject;
use crate::query::filter::{Filter, FilterValue};
use crate::query::order::{Direction, Order};
use crate::query::{query_by_id, query_total_count, ID};
use crate::types::{ScalarType, TypeData};
use crate::utils::extract_value::extract;

const BOOLEAN_TRUE: i64 = 1;

#[derive(FromRow, Deserialize, PartialEq, Eq)]
pub struct ModelMember {
    pub id: String,
    pub model_id: String,
    pub model_idx: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub key: bool,
    pub created_at: DateTime<Utc>,
}

pub struct ModelDataObject {
    pub name: String,
    pub type_name: String,
    pub type_mapping: TypeMapping,
    pub where_input: WhereInputObject,
    pub order_input: OrderInputObject,
}

impl ModelDataObject {
    pub fn new(name: String, type_name: String, type_mapping: TypeMapping) -> Self {
        let where_input = WhereInputObject::new(type_name.as_str(), &type_mapping);
        let order_input = OrderInputObject::new(type_name.as_str(), &type_mapping);
        Self { name, type_name, type_mapping, where_input, order_input }
    }
}

impl ObjectTrait for ModelDataObject {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }

    fn sub_fields(&self) -> Option<Vec<Field>> {
        let mut fields = Vec::new();
        fields.push(entity_field());
        fields.extend(nested_type_fields(self.type_name.as_str(), &self.type_mapping));

        Some(fields)
    }

    fn input_objects(&self) -> Option<Vec<InputObject>> {
        Some(vec![self.where_input.input_object(), self.order_input.input_object()])
    }

    fn enum_objects(&self) -> Option<Vec<Enum>> {
        self.order_input.enum_objects()
    }

    fn child_objects(&self) -> Option<Vec<Object>> {
        let child_objects: Vec<Object> = self
            .type_mapping
            .iter()
            .filter_map(|(_, type_data)| {
                if let TypeData::Nested((type_ref, nested_mapping)) = type_data {
                    let mut object = Object::new(type_ref.to_string());

                    for (field_name, type_data) in nested_mapping {
                        let field_name = field_name.clone();

                        let field =
                            Field::new(field_name.to_string(), type_data.type_ref(), move |ctx| {
                                let field_name = field_name.clone();

                                FieldFuture::new(async move {
                                    match ctx.parent_value.try_to_value()? {
                                        Value::Object(values) => {
                                            Ok(Some(values.get(&field_name).unwrap().clone())) // safe unwrap
                                        }
                                        _ => Err("incorrect value, requires Value::Object".into()),
                                    }
                                })
                            });

                        object = object.field(field);
                    }

                    Some(object)
                } else {
                    None
                }
            })
            .collect();

        Some(child_objects)
    }

    fn resolve_many(&self) -> Option<Field> {
        let type_name = self.type_name.clone();
        let type_mapping = self.type_mapping.clone();
        let where_mapping = self.where_input.type_mapping.clone();
        let field_name = format!("{}Models", self.name());
        let field_type = format!("{}Connection", self.type_name());

        let mut field = Field::new(field_name, TypeRef::named(field_type), move |ctx| {
            let type_mapping = type_mapping.clone();
            let where_mapping = where_mapping.clone();
            let type_name = type_name.clone();

            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let order = parse_order_argument(&ctx);
                let filters = parse_where_argument(&ctx, &where_mapping)?;
                let connection = parse_connection_arguments(&ctx)?;

                let data =
                    models_data_query(&mut conn, &type_name, &order, &filters, &connection).await?;

                let total_count = query_total_count(&mut conn, &type_name, &filters).await?;
                let connection = model_connection(&data, &type_mapping, total_count)?;

                Ok(Some(Value::Object(connection)))
            })
        });

        // Add relay connection fields (first, last, before, after, where)
        field = connection_arguments(field);
        // field = where_argument(field, self.type_name());
        // field = order_argument(field, self.type_name());

        Some(field)
    }
}

fn entity_field() -> Field {
    Field::new("entity", TypeRef::named("Entity"), |ctx| {
        FieldFuture::new(async move {
            match ctx.parent_value.try_to_value()? {
                Value::Object(indexmap) => {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let entity_id = extract::<String>(indexmap, "entity_id")?;
                    let entity: Entity =
                        query_by_id(&mut conn, "entities", ID::Str(entity_id)).await?;
                    let result = EntityObject::value_mapping(entity);

                    Ok(Some(Value::Object(result)))
                }
                _ => Err("incorrect value, requires Value::Object".into()),
            }
        })
    })
}

// FIXME: recursively handle deeply nested types, rn just handling one level deep
fn nested_type_fields(root_type: &str, type_mapping: &TypeMapping) -> Vec<Field> {
    type_mapping
        .iter()
        .filter_map(|(field_name, type_data)| {
            if type_data.is_nested() {
                let root_type = root_type.to_string();
                let type_data = type_data.clone();

                Some(Field::new(field_name.to_string(), type_data.type_ref(), move |ctx| {
                    let root_type = root_type.clone();
                    let type_data = type_data.clone();

                    FieldFuture::new(async move {
                        match ctx.parent_value.try_to_value()? {
                            Value::Object(indexmap) => {
                                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                                let type_ref = type_data.type_ref();
                                let table_name = format!("{}${}", root_type, type_ref); // FIXME: this assumes one level nesting
                                let entity_id = extract::<String>(indexmap, "entity_id")?;

                                let result = model_data_by_id_query(
                                    &mut conn,
                                    &table_name,
                                    &entity_id,
                                    &type_data.type_mapping().unwrap(),
                                )
                                .await?;

                                Ok(Some(Value::Object(result)))
                            }
                            _ => Err("incorrect value, requires Value::Object".into()),
                        }
                    })
                }))
            } else {
                None
            }
        })
        .collect()
}

pub async fn model_data_by_id_query(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    entity_id: &str,
    type_mapping: &TypeMapping,
) -> sqlx::Result<ValueMapping> {
    let query = format!("SELECT * FROM {} WHERE entity_id = '{}'", table_name, entity_id);
    let row = sqlx::query(&query).fetch_one(conn).await?;
    value_mapping_from_row(&row, type_mapping)
}

pub async fn models_data_query(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    order: &Option<Order>,
    filters: &Vec<Filter>,
    connection: &ConnectionArguments,
) -> sqlx::Result<Vec<SqliteRow>> {
    let mut query = format!("SELECT * FROM {}", table_name);
    let mut conditions = Vec::new();

    // Handle after cursor if exists
    if let Some(after_cursor) = &connection.after {
        match decode_cursor(after_cursor.clone()) {
            Ok((created_at, id)) => {
                conditions.push(format!("(created_at, entity_id) < ('{}', '{}')", created_at, id));
            }
            Err(_) => return Err(sqlx::Error::Decode("Invalid after cursor format".into())),
        }
    }

    // Handle before cursor if exists
    if let Some(before_cursor) = &connection.before {
        match decode_cursor(before_cursor.clone()) {
            Ok((created_at, id)) => {
                conditions.push(format!("(created_at, entity_id) > ('{}', '{}')", created_at, id));
            }
            Err(_) => return Err(sqlx::Error::Decode("Invalid before cursor format".into())),
        }
    }

    // Handle filters
    for filter in filters {
        let condition = match filter.value {
            FilterValue::Int(i) => format!("{} {} {}", filter.field, filter.comparator, i),
            FilterValue::String(ref s) => format!("{} {} '{}'", filter.field, filter.comparator, s),
        };

        conditions.push(condition);
    }

    // Combine conditions query
    if !conditions.is_empty() {
        query.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
    }

    // Handle order and limit
    // NOTE: Order is determiined by the `order` param if provided, otherwise it's inferred from the
    // `first` or `last` param. Explicity ordering take precedence
    let limit = connection.first.or(connection.last).unwrap_or(DEFAULT_LIMIT);
    let (column, direction) = if let Some(order) = order {
        let column = format!("external_{}", order.field);
        (
            column,
            match order.direction {
                Direction::Asc => "ASC",
                Direction::Desc => "DESC",
            },
        )
    } else {
        // if no order specified default to created_at
        ("created_at".to_string(), if connection.first.is_some() { "DESC" } else { "ASC" })
    };

    query.push_str(&format!(" ORDER BY {column} {direction} LIMIT {limit}"));

    sqlx::query(&query).fetch_all(conn).await
}

// TODO: make `connection_output()` more generic. Currently, `model_connection()` method
// required as we need to explicity add `entity_id` to each edge.
pub fn model_connection(
    data: &[SqliteRow],
    types: &TypeMapping,
    total_count: i64,
) -> sqlx::Result<ValueMapping> {
    let model_edges = data
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
        (Name::new("edges"), Value::List(model_edges?)),
        // TODO: add pageInfo
    ]))
}

fn value_mapping_from_row(row: &SqliteRow, types: &TypeMapping) -> sqlx::Result<ValueMapping> {
    types
        .iter()
        .filter(|(_, type_data)| type_data.is_simple())
        .map(|(field_name, type_data)| {
            Ok((Name::new(field_name), fetch_value(row, field_name, &type_data.type_ref().to_string())?))
        })
        .collect::<sqlx::Result<ValueMapping>>()
}

fn fetch_value(row: &SqliteRow, field_name: &str, field_type: &str) -> sqlx::Result<Value> {
    let column_name = format!("external_{}", field_name);
    match CairoType::from_str(field_type) {
        Ok(CairoType::Bool) => fetch_boolean(row, &column_name),
        Ok(ty) if ty.to_sql_type() == "INTEGER" => fetch_numeric(row, &column_name),
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
    model_id: &str,
) -> sqlx::Result<TypeMapping> {
    let model_members: Vec<ModelMember> = sqlx::query_as(
        r#"
        SELECT
            id,
            model_id,
            model_idx,
            name,
            type AS ty,
            key,
            created_at
        from model_members WHERE model_id = ?
        "#,
    )
    .bind(model_id)
    .fetch_all(conn)
    .await?;

    let (root_members, nested_members): (Vec<&ModelMember>, Vec<&ModelMember>) =
        model_members.iter().partition(|member| member.model_idx == 0);

    let type_mapping: TypeMapping = root_members
        .iter()
        .map(|member| {
            let type_data = match CairoType::from_str(&member.ty) {
                Ok(_) => TypeData::Simple(TypeRef::named(member.ty.clone())),
                _ => parse_type(&member.model_id, &member.ty, &nested_members),
            };

            (Name::new(&member.name), type_data)
        })
        .collect();

    Ok(type_mapping)
}

fn parse_type(target_id: &str, target_type: &str, nested_members: &Vec<&ModelMember>) -> TypeData {
    let nested_mapping: TypeMapping = nested_members
        .iter()
        .filter_map(|member| {
            // search for target type in nested members
            if target_id == member.model_id && member.id.ends_with(target_type) {
                let type_data = match CairoType::from_str(&member.ty) {
                    Ok(_) => TypeData::Simple(TypeRef::named(member.ty.clone())),
                    _ => parse_type(&member.model_id, &member.ty, nested_members),
                };

                Some((Name::new(&member.name), type_data))
            } else {
                None
            }
        })
        .collect();

    TypeData::Nested((TypeRef::named(target_type), nested_mapping))
}
