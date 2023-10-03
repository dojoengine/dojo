use std::str::FromStr;

use async_graphql::dynamic::{Enum, Field, FieldFuture, InputObject, Object, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use dojo_types::primitive::Primitive;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Pool, Row, Sqlite};
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
use crate::query::{query_by_id, query_total_count};
use crate::types::TypeData;
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

    fn input_objects(&self) -> Option<Vec<InputObject>> {
        Some(vec![self.where_input.input_object(), self.order_input.input_object()])
    }

    fn enum_objects(&self) -> Option<Vec<Enum>> {
        self.order_input.enum_objects()
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
        field = where_argument(field, self.type_name());
        field = order_argument(field, self.type_name());

        Some(field)
    }

    fn objects(&self) -> Vec<Object> {
        let mut path_array = vec![self.type_name().to_string()];
        let mut objects = data_objects(self.type_name(), self.type_mapping(), &mut path_array);

        // root object requires entity_field association
        let mut root = objects.pop().unwrap();
        root = root.field(entity_field());

        objects.push(root);
        objects
    }
}

fn data_objects(
    type_name: &str,
    type_mapping: &TypeMapping,
    path_array: &mut Vec<String>,
) -> Vec<Object> {
    let mut objects = Vec::<Object>::new();

    for (_, type_data) in type_mapping {
        if let TypeData::Nested((nested_type, nested_mapping)) = type_data {
            path_array.push(nested_type.to_string());
            objects.extend(data_objects(
                &nested_type.to_string(),
                nested_mapping,
                &mut path_array.clone(),
            ));
        }
    }

    objects.push(object(type_name, type_mapping, path_array));
    objects
}

pub fn object(type_name: &str, type_mapping: &TypeMapping, path_array: &[String]) -> Object {
    let mut object = Object::new(type_name);

    for (field_name, type_data) in type_mapping.clone() {
        let table_name = path_array.join("$");

        let field = Field::new(field_name.to_string(), type_data.type_ref(), move |ctx| {
            let field_name = field_name.clone();
            let type_data = type_data.clone();
            let table_name = table_name.clone();

            // Field resolver for nested types
            if let TypeData::Nested((_, nested_mapping)) = type_data {
                return FieldFuture::new(async move {
                    match ctx.parent_value.try_to_value()? {
                        Value::Object(indexmap) => {
                            let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                            let entity_id = extract::<String>(indexmap, "entity_id")?;

                            // TODO: remove subqueries and use JOIN in parent query
                            let result = model_data_by_id_query(
                                &mut conn,
                                &table_name,
                                &entity_id,
                                &nested_mapping,
                            )
                            .await?;

                            Ok(Some(Value::Object(result)))
                        }
                        _ => Err("incorrect value, requires Value::Object".into()),
                    }
                });
            }

            // Field resolver for simple types and model union
            FieldFuture::new(async move {
                if let Some(value) = ctx.parent_value.as_value() {
                    return match value {
                        Value::Object(value_mapping) => {
                            Ok(Some(value_mapping.get(&field_name).unwrap().clone()))
                        }
                        _ => Err("Incorrect value, requires Value::Object".into()),
                    };
                }

                // Catch model union resolutions, async-graphql sends union types as IndexMap<Name,
                // ConstValue>
                if let Some(value_mapping) = ctx.parent_value.downcast_ref::<ValueMapping>() {
                    return Ok(Some(value_mapping.get(&field_name).unwrap().clone()));
                }

                Err("Field resolver only accepts Value or IndexMap".into())
            })
        });

        object = object.field(field);
    }

    object
}

fn entity_field() -> Field {
    Field::new("entity", TypeRef::named("Entity"), |ctx| {
        FieldFuture::new(async move {
            match ctx.parent_value.try_to_value()? {
                Value::Object(indexmap) => {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let entity_id = extract::<String>(indexmap, "entity_id")?;
                    let entity: Entity = query_by_id(&mut conn, "entities", &entity_id).await?;
                    let result = EntityObject::value_mapping(entity);

                    Ok(Some(Value::Object(result)))
                }
                _ => Err("incorrect value, requires Value::Object".into()),
            }
        })
    })
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

// TODO: make `connection_output()` more generic.
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
            let value_mapping = value_mapping_from_row(row, types)?;

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
    let mut value_mapping = types
        .iter()
        .filter(|(_, type_data)| type_data.is_simple())
        .map(|(field_name, type_data)| {
            let column_name = format!("external_{}", field_name);
            Ok((
                Name::new(field_name),
                fetch_value(row, &column_name, &type_data.type_ref().to_string())?,
            ))
        })
        .collect::<sqlx::Result<ValueMapping>>()?;

    // entity_id column is a foreign key associating back to original entity and is not prefixed
    // with `external_`
    value_mapping.insert(Name::new("entity_id"), fetch_value(row, "entity_id", TypeRef::STRING)?);

    Ok(value_mapping)
}

fn fetch_value(row: &SqliteRow, column_name: &str, field_type: &str) -> sqlx::Result<Value> {
    match Primitive::from_str(field_type) {
        // fetch boolean
        Ok(Primitive::Bool(_)) => {
            Ok(Value::from(matches!(row.try_get::<i64, &str>(column_name)?, BOOLEAN_TRUE)))
        }
        // fetch integer
        Ok(ty) if ty.to_sql_type() == "INTEGER" => {
            row.try_get::<i64, &str>(column_name).map(Value::from)
        }
        // fetch string
        _ => row.try_get::<String, &str>(column_name).map(Value::from),
    }
}
