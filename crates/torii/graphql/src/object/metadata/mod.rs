use async_graphql::connection::PageInfo;
use async_graphql::dynamic::{Field, FieldFuture, TypeRef};
use async_graphql::{Name, Value};
use convert_case::{Case, Casing};
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, Row, Sqlite};

use super::connection::page_info::PageInfoObject;
use super::connection::{connection_arguments, cursor, parse_connection_arguments};
use super::ObjectTrait;
use crate::constants::{
    ID_COLUMN, JSON_COLUMN, METADATA_NAMES, METADATA_TABLE, METADATA_TYPE_NAME,
};
use crate::mapping::METADATA_TYPE_MAPPING;
use crate::query::data::{count_rows, fetch_multiple_rows, fetch_world_address};
use crate::query::value_mapping_from_row;
use crate::types::{TypeMapping, ValueMapping};

pub mod content;
pub mod social;

pub struct MetadataObject;

impl MetadataObject {
    fn row_types(&self) -> TypeMapping {
        let mut row_types = self.type_mapping().clone();
        row_types.remove("worldAddress");
        row_types
    }
}

impl ObjectTrait for MetadataObject {
    fn name(&self) -> (&str, &str) {
        METADATA_NAMES
    }

    fn type_name(&self) -> &str {
        METADATA_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &METADATA_TYPE_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(METADATA_TABLE)
    }

    fn resolve_one(&self) -> Option<Field> {
        None
    }

    fn resolve_many(&self) -> Option<Field> {
        let table_name = self.table_name().unwrap().to_string();
        let row_types = self.row_types();

        let mut field = Field::new(
            self.name().1,
            TypeRef::named(format!("{}Connection", self.type_name())),
            move |ctx| {
                let row_types = row_types.clone();
                let table_name = table_name.to_string();

                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let connection = parse_connection_arguments(&ctx)?;
                    let total_count = count_rows(&mut conn, &table_name, &None, &None).await?;
                    let (data, page_info) = fetch_multiple_rows(
                        &mut conn,
                        &table_name,
                        ID_COLUMN,
                        &None,
                        &None,
                        &None,
                        &connection,
                        total_count,
                    )
                    .await?;
                    let world_address = fetch_world_address(&mut conn).await?;

                    // convert json field to value_mapping expected by content object
                    let results = metadata_connection_output(
                        &data,
                        &row_types,
                        total_count,
                        page_info,
                        &world_address,
                    )?;

                    Ok(Some(Value::Object(results)))
                })
            },
        );

        field = connection_arguments(field);

        Some(field)
    }
}

// NOTE: need to generalize `connection_output` or maybe preprocess to support both predefined
// objects AND dynamic model objects
fn metadata_connection_output(
    data: &[SqliteRow],
    row_types: &TypeMapping,
    total_count: i64,
    page_info: PageInfo,
    world_address: &String,
) -> sqlx::Result<ValueMapping> {
    let edges = data
        .iter()
        .map(|row| {
            let order = row.try_get::<String, &str>(ID_COLUMN)?;
            let cursor = cursor::encode(&order, &order);
            let mut value_mapping = value_mapping_from_row(row, row_types, false)?;
            value_mapping.insert(Name::new("worldAddress"), Value::from(world_address));

            let json_str = row.try_get::<String, &str>(JSON_COLUMN)?;
            let serde_value = serde_json::from_str(&json_str).unwrap_or_default();

            let content = ValueMapping::from([
                extract_str_mapping("name", &serde_value),
                extract_str_mapping("description", &serde_value),
                extract_str_mapping("website", &serde_value),
                extract_str_mapping("icon_uri", &serde_value),
                extract_str_mapping("cover_uri", &serde_value),
                extract_socials_mapping("socials", &serde_value),
            ]);

            value_mapping.insert(Name::new("content"), Value::Object(content));

            let edge = ValueMapping::from([
                (Name::new("node"), Value::Object(value_mapping)),
                (Name::new("cursor"), Value::String(cursor)),
            ]);

            Ok(Value::Object(edge))
        })
        .collect::<sqlx::Result<Vec<Value>>>();

    Ok(ValueMapping::from([
        (Name::new("totalCount"), Value::from(total_count)),
        (Name::new("edges"), Value::List(edges?)),
        (Name::new("pageInfo"), PageInfoObject::value(page_info)),
    ]))
}

fn extract_str_mapping(name: &str, serde_value: &serde_json::Value) -> (Name, Value) {
    let name_lower_camel = name.to_case(Case::Camel);
    if let Some(serde_json::Value::String(str)) = serde_value.get(name) {
        (Name::new(name_lower_camel), Value::String(str.to_owned()))
    } else {
        (Name::new(name_lower_camel), Value::Null)
    }
}

fn extract_socials_mapping(name: &str, serde_value: &serde_json::Value) -> (Name, Value) {
    if let Some(serde_json::Value::Object(obj)) = serde_value.get(name) {
        let list = obj
            .iter()
            .map(|(social_name, social_url)| {
                Value::Object(ValueMapping::from([
                    (Name::new("name"), Value::String(social_name.to_string())),
                    (Name::new("url"), Value::String(social_url.as_str().unwrap().to_string())),
                ]))
            })
            .collect::<Vec<Value>>();

        return (Name::new(name), Value::List(list));
    }

    (Name::new(name), Value::List(vec![]))
}
