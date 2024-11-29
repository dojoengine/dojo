use std::str::FromStr;

use async_graphql::dynamic::TypeRef;
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use convert_case::{Case, Casing};
use dojo_types::primitive::{Primitive, SqlType};
use dojo_types::schema::Ty;
use regex::Regex;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use torii_core::constants::SQL_FELT_DELIMITER;

use crate::constants::{
    BOOLEAN_TRUE, ENTITY_ID_COLUMN, EVENT_MESSAGE_ID_COLUMN, INTERNAL_ENTITY_ID_KEY,
};
use crate::types::{TypeData, TypeMapping, ValueMapping};

pub mod data;
pub mod filter;
pub mod order;

pub fn build_type_mapping(namespace: &str, schema: &Ty) -> TypeMapping {
    let model = schema.as_struct().unwrap();

    model
        .children
        .iter()
        .map(|member| {
            let type_data = member_to_type_data(namespace, &member.ty);
            (Name::new(&member.name), type_data)
        })
        .collect()
}

fn member_to_type_data(namespace: &str, schema: &Ty) -> TypeData {
    // TODO: convert sql -> Ty directly
    match schema {
        Ty::Primitive(primitive) => TypeData::Simple(TypeRef::named(primitive.to_string())),
        Ty::ByteArray(_) => TypeData::Simple(TypeRef::named("ByteArray")),
        Ty::Array(array) => TypeData::List(Box::new(member_to_type_data(namespace, &array[0]))),
        // Enums that do not have a nested member are considered as a simple Enum
        Ty::Enum(enum_)
            if enum_
                .options
                .iter()
                .all(|o| if let Ty::Tuple(t) = &o.ty { t.is_empty() } else { false }) =>
        {
            TypeData::Simple(TypeRef::named("Enum"))
        }
        _ => parse_nested_type(namespace, schema),
    }
}

fn parse_nested_type(namespace: &str, schema: &Ty) -> TypeData {
    let type_mapping: TypeMapping = match schema {
        Ty::Struct(s) => s
            .children
            .iter()
            .map(|member| {
                let type_data = member_to_type_data(namespace, &member.ty);
                (Name::new(&member.name), type_data)
            })
            .collect(),
        Ty::Enum(e) => {
            let mut type_mapping = e
                .options
                .iter()
                .filter_map(|option| {
                    // ignore unit type variants
                    if let Ty::Tuple(t) = &option.ty {
                        if t.is_empty() {
                            return None;
                        }
                    }

                    let type_data = member_to_type_data(namespace, &option.ty);
                    Some((Name::new(&option.name), type_data))
                })
                .collect::<TypeMapping>();

            type_mapping.insert(Name::new("option"), TypeData::Simple(TypeRef::named("Enum")));
            type_mapping
        }
        _ => return TypeData::Simple(TypeRef::named(schema.name())),
    };

    let name: String = format!("{}_{}", namespace, schema.name());
    // sanitizes the member type string
    // for eg. Position_Array<Vec2> -> Position_ArrayVec2
    // Position_(u8, Vec2) -> Position_u8Vec2
    let re = Regex::new(r"[, ()<>-]").unwrap();
    let sanitized_member_type_name = re.replace_all(&name, "");
    TypeData::Nested((TypeRef::named(sanitized_member_type_name), type_mapping))
}

fn remove_hex_leading_zeros(value: Value) -> Value {
    if let Value::String(str_val) = &value {
        if !str_val.starts_with("0x") {
            return value;
        }
        let hex_part = str_val.trim_start_matches("0x");
        let trimmed_hex = hex_part.trim_start_matches('0');
        Value::String(format!("0x{:0>1}", trimmed_hex))
    } else {
        value
    }
}

pub fn value_mapping_from_row(
    row: &SqliteRow,
    types: &TypeMapping,
    is_external: bool,
) -> sqlx::Result<ValueMapping> {
    println!("types: {:?}", types);
    fn build_value_mapping(
        row: &SqliteRow,
        types: &TypeMapping,
        prefix: &str,
        is_external: bool,
    ) -> sqlx::Result<ValueMapping> {
        let mut value_mapping = ValueMapping::new();

        for (field_name, type_data) in types {
            let column_name = if prefix.is_empty() {
                field_name.to_string()
            } else {
                format!("{}.{}", prefix, field_name)
            };

            match type_data {
                TypeData::Simple(type_ref) => {
                    let mut value = fetch_value(row, &column_name, &type_ref.to_string(), is_external)?;

                    // handles felt arrays stored as string (ex: keys)
                    if let (TypeRef::List(_), Value::String(s)) = (type_ref, &value) {
                        let mut felts: Vec<_> = s.split(SQL_FELT_DELIMITER).map(Value::from).collect();
                        felts.pop(); // removes empty item
                        value = Value::List(felts);
                    }

                    value_mapping.insert(Name::new(field_name), value);
                }
                TypeData::List(inner) => {
                    let value = fetch_value(row, &column_name, "String", is_external)?;
                    if let Value::String(json_str) = value {
                        let array_value: Value = serde_json::from_str(&json_str)
                            .map_err(|e| sqlx::Error::Protocol(format!("JSON parse error: {}", e)))?;
                        value_mapping.insert(Name::new(field_name), array_value);
                    }
                }
                TypeData::Nested((_, nested_mapping)) => {
                    let nested_values = build_value_mapping(
                        row,
                        nested_mapping,
                        &column_name,
                        is_external,
                    )?;
                    value_mapping.insert(Name::new(field_name), Value::Object(nested_values));
                }
            }
        }

        Ok(value_mapping)
    }

    let mut value_mapping = build_value_mapping(row, types, "", is_external)?;

    // Add internal entity ID if present
    if let Ok(entity_id) = row.try_get::<String, &str>(ENTITY_ID_COLUMN) {
        value_mapping.insert(Name::new(INTERNAL_ENTITY_ID_KEY), Value::from(entity_id));
    } else if let Ok(event_message_id) = row.try_get::<String, &str>(EVENT_MESSAGE_ID_COLUMN) {
        value_mapping.insert(Name::new(INTERNAL_ENTITY_ID_KEY), Value::from(event_message_id));
    }

    Ok(value_mapping)
}

fn fetch_value(
    row: &SqliteRow,
    field_name: &str,
    type_name: &str,
    is_external: bool,
) -> sqlx::Result<Value> {
    let mut column_name = if !is_external {
        format!("internal_{}", field_name)
    } else {
        field_name.to_string()
    };

    // for enum options, remove the ".option" suffix to get the variant
    // through the enum itself field name
    if type_name == "Enum" && column_name.ends_with(".option") {
        column_name = column_name.trim_end_matches(".option").to_string();
    }

    match Primitive::from_str(type_name) {
        // fetch boolean
        Ok(Primitive::Bool(_)) => {
            Ok(Value::from(matches!(row.try_get::<i64, &str>(&column_name)?, BOOLEAN_TRUE)))
        }
        // fetch integer/string base on sql type
        Ok(ty) => match ty.to_sql_type() {
            SqlType::Integer => row.try_get::<i64, &str>(&column_name).map(Value::from),
            SqlType::Text => Ok(remove_hex_leading_zeros(
                row.try_get::<String, &str>(&column_name).map(Value::from)?,
            )),
        },
        // fetch everything else
        _ => {
            let value = match type_name {
                "DateTime" => {
                    let dt = row
                        .try_get::<DateTime<Utc>, &str>(&column_name)
                        .expect("Should be a stored as UTC Datetime")
                        .to_rfc3339();
                    Value::from(dt)
                }
                _ => {
                    let s = row.try_get::<String, &str>(&column_name)?;
                    Value::from(s)
                }
            };
            Ok(value)
        }
    }
}
