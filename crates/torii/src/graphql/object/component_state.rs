use async_graphql::dynamic::{Field, FieldFuture, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Pool, QueryBuilder, Row, Sqlite};

use super::connection::{connection_input, encode_cursor, parse_arguments};
use super::query::{query_total_count, Order};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::object::entity::{Entity, EntityObject};
use crate::graphql::object::query::{query_by_id, ID};
use crate::graphql::types::ScalarType;
use crate::graphql::utils::extract_value::extract;

const BOOLEAN_TRUE: i64 = 1;
const ENTITY_ID: &str = "entity_id";

#[derive(FromRow, Deserialize)]
pub struct ComponentMembers {
    pub component_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub slot: i64,
    pub offset: i64,
    pub created_at: DateTime<Utc>,
}

pub struct ComponentStateObject {
    pub name: String,
    pub type_name: String,
    pub type_mapping: TypeMapping,
}

impl ComponentStateObject {
    pub fn new(name: String, type_name: String, type_mapping: TypeMapping) -> Self {
        Self { name, type_name, type_mapping }
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

    fn resolve_many(&self) -> Option<Field> {
        let name = self.name.clone();
        let type_mapping = self.type_mapping.clone();
        let field_name = format!("{}Components", self.name());
        let field_type = format!("{}Connection", self.type_name());

        let mut field = Field::new(field_name, TypeRef::named(field_type), move |ctx| {
            let type_mapping = type_mapping.clone();
            let name = name.clone();

            FieldFuture::new(async move {
                let (limit, offset, order) = parse_arguments(&ctx)?;
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let table_name = format!("external_{}", name);
                let total_count = query_total_count(&mut conn, &table_name).await?;
                let data =
                    component_states_query(&mut conn, &table_name, order, limit, offset).await?;
                let connection = component_connection(&data, &type_mapping, total_count)?;

                Ok(Some(Value::Object(connection)))
            })
        });

        // Add relay connection fields (first, last, before, after)
        field = connection_input(field);

        // TODO: type mapping also act as filters, add this to `where: nameWhereInput`
        // field = self
        //     .type_mapping()
        //     .into_iter()
        //     .fold(field, |field, (name, ty)| {
        //         let ty = ty.clone();
        //         // we want to be able to return entity_id in component queries
        //         // but don't need this as a filter parameter
        //         match name.as_str() {
        //             ENTITY_ID => field,
        //             _ => field.argument(InputValue::new(name.as_str(), ty)),
        //         }
        //     })
        //     .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT)));

        Some(field)
    }
}

fn entity_field() -> Field {
    Field::new("entity", TypeRef::named("Entity"), |ctx| {
        FieldFuture::new(async move {
            match ctx.parent_value.try_to_value()? {
                Value::Object(indexmap) => {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = extract::<String>(indexmap, ENTITY_ID)?;
                    let entity: Entity = query_by_id(&mut conn, "entities", ID::Str(id)).await?;
                    let result = EntityObject::value_mapping(entity);

                    Ok(Some(Value::Object(result)))
                }
                _ => Err("incorrect value, requires Value::Object".into()),
            }
        })
    })
}

// fn parse_inputs(
//     ctx: &ResolverContext<'_>,
//     type_mapping: &TypeMapping,
// ) -> async_graphql::Result<(ComponentFilters, u64), async_graphql::Error> {
//     let mut filters: ComponentFilters = ComponentFilters::new();

//     // parse inputs based on field type mapping
//     for (name, ty) in type_mapping.iter() {
//         let input_option = ctx.args.try_get(name.as_str());

//         if let Ok(input) = input_option {
//             let input_str = match ScalarType::from_str(ty.to_string())? {
//                 scalar if scalar.is_numeric_type() => input.u64()?.to_string(),
//                 _ => input.string()?.to_string(),
//             };

//             filters.insert(name.to_string(), input_str);
//         }
//     }

//     let limit = ctx.args.try_get("limit").and_then(|limit| limit.u64()).unwrap_or(DEFAULT_LIMIT);

//     Ok((filters, limit))
// }

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
    order: Order,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<SqliteRow>> {
    // FIXME?: Possible performance impact using row_number window function for large datasets, can migrate to use
    // (created_at, id) for cursor. However, created_at also doesn't reflect proper chronological ordering as multiple
    // entires could have the same timestamp. Possible fix is to include block_number as part of component data
    // and used as cursor.
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        "SELECT *, row_number() OVER (ORDER BY created_at DESC) AS row_number FROM ",
    );
    builder.push(table_name);
    builder.push(" ORDER BY created_at ").push(order);
    builder.push(" LIMIT ").push(limit);
    builder.push(" OFFSET ").push(offset);

    builder.build().fetch_all(conn).await
}

// TODO: make `connection_output()` more generic. Currently, `component_connection()` method required as we
// need to explicity add `entity_id` to each edge.
fn component_connection(
    data: &Vec<SqliteRow>,
    types: &TypeMapping,
    total_count: i64,
) -> sqlx::Result<ValueMapping> {
    let component_edges = data
        .iter()
        .map(|row| {
            // Fetch entity_id and offset manually as they're not part of component memebers
            let row_number = fetch_numeric(row, "row_number")?.to_string();
            let entity_id = fetch_string(row, "entity_id")?;

            let mut value_mapping = value_mapping_from_row(row, types)?;
            value_mapping.insert(Name::new("entity_id"), entity_id);

            let mut edge = ValueMapping::new();
            edge.insert(Name::new("node"), Value::Object(value_mapping));
            edge.insert(Name::new("cursor"), Value::String(encode_cursor(row_number.clone())));

            Ok(Value::Object(edge))
        })
        .collect::<sqlx::Result<Vec<Value>>>();

    Ok(ValueMapping::from([
        (Name::new("totalCount"), Value::from(total_count)),
        (Name::new("edges"), Value::List(component_edges?)),
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
                    slot,
                    offset,
                    created_at
                FROM component_members WHERE component_id = ?
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
