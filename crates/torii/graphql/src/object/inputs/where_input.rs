use std::str::FromStr;

use async_graphql::dynamic::{
    Field, InputObject, InputValue, ResolverContext, TypeRef, ValueAccessor,
};
use async_graphql::{Error as GqlError, Name, Result};
use dojo_types::primitive::{Primitive, SqlType};
use strum::IntoEnumIterator;

use super::InputObjectTrait;
use crate::object::TypeMapping;
use crate::query::filter::{parse_filter, Comparator, Filter, FilterValue};
use crate::types::TypeData;

#[derive(Debug)]
pub struct WhereInputObject {
    pub type_name: String,
    pub type_mapping: TypeMapping,
}

impl WhereInputObject {
    fn build_mapping(prefix: &str, types: &TypeMapping) -> TypeMapping {
        types
            .iter()
            .filter(|(_, type_data)| !type_data.is_list())
            .flat_map(|(type_name, type_data)| {
                let field_name = if prefix.is_empty() {
                    type_name.to_string()
                } else {
                    format!("{}_{}", prefix.replace('.', "_"), type_name)
                };

                if type_data.type_ref() == TypeRef::named("Enum")
                    || type_data.type_ref() == TypeRef::named("bool")
                {
                    return vec![(Name::new(field_name), type_data.clone())];
                }

                // Handle nested types
                if type_data.is_nested() {
                    if let TypeData::Nested((_, nested_types)) = type_data {
                        return nested_types
                            .iter()
                            .flat_map(|(nested_name, nested_type)| {
                                if !nested_type.is_nested() || nested_type.type_ref() == TypeRef::named("Enum") {
                                    let nested_field = format!("{}_{}", field_name, nested_name);
                                    return Comparator::iter().fold(
                                        vec![(Name::new(&nested_field), nested_type.clone())],
                                        |mut acc, comparator| {
                                            let name = format!("{}{}", nested_field, comparator.as_ref());
                                            match comparator {
                                                Comparator::In | Comparator::NotIn => acc.push((
                                                    Name::new(name),
                                                    TypeData::List(Box::new(nested_type.clone())),
                                                )),
                                                _ => {
                                                    acc.push((Name::new(name), nested_type.clone()));
                                                }
                                            }
                                            acc
                                        },
                                    );
                                }
                                
                                if let TypeData::Nested((_, further_nested_types)) = nested_type {
                                    let new_prefix = format!("{}_{}", field_name, nested_name);
                                    return Self::build_mapping(&new_prefix, further_nested_types)
                                        .into_iter()
                                        .collect();
                                }
                                
                                vec![]
                            })
                            .collect();
                    }
                }

                // Handle regular fields with comparators
                Comparator::iter().fold(
                    vec![(Name::new(&field_name), type_data.clone())],
                    |mut acc, comparator| {
                        let name = format!("{}{}", field_name, comparator.as_ref());
                        match comparator {
                            Comparator::In | Comparator::NotIn => acc.push((
                                Name::new(name),
                                TypeData::List(Box::new(type_data.clone())),
                            )),
                            _ => {
                                acc.push((Name::new(name), type_data.clone()));
                            }
                        }
                        acc
                    },
                )
            })
            .collect()
    }

    pub fn new(type_name: &str, object_types: &TypeMapping) -> Self {
        let where_mapping = Self::build_mapping("", object_types);

        Self { type_name: format!("{}WhereInput", type_name), type_mapping: where_mapping }
    }
}

impl InputObjectTrait for WhereInputObject {
    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }

    fn input_object(&self) -> InputObject {
        self.type_mapping.iter().fold(InputObject::new(self.type_name()), |acc, (ty_name, ty)| {
            acc.field(InputValue::new(ty_name.to_string(), ty.type_ref()))
        })
    }
}

pub fn where_argument(field: Field, type_name: &str) -> Field {
    field.argument(InputValue::new("where", TypeRef::named(format!("{}WhereInput", type_name))))
}

pub fn parse_where_argument(
    ctx: &ResolverContext<'_>,
    where_mapping: &TypeMapping,
) -> Result<Option<Vec<Filter>>> {
    ctx.args.get("where").map_or(Ok(None), |where_input| {
        let input_object = where_input.object()?;
        where_mapping
            .iter()
            .filter_map(|(type_name, type_data)| {
                input_object.get(type_name).map(|input| match type_data {
                    TypeData::Simple(_) => {
                        if type_data.type_ref() == TypeRef::named("Enum") {
                            let value = input.string().unwrap();
                            return Ok(Some(parse_filter(
                                type_name,
                                FilterValue::String(value.to_string()),
                            )));
                        }

                        let primitive = Primitive::from_str(&type_data.type_ref().to_string())?;
                        let filter_value = match primitive.to_sql_type() {
                            SqlType::Integer => parse_integer(input, type_name, primitive)?,
                            SqlType::Text => parse_string(input, type_name, primitive)?,
                        };

                        Ok(Some(parse_filter(type_name, filter_value)))
                    }
                    TypeData::List(inner) => {
                        let list = input.list()?;
                        let values = list
                            .iter()
                            .map(|value| {
                                let primitive = Primitive::from_str(&inner.type_ref().to_string())?;
                                match primitive.to_sql_type() {
                                    SqlType::Integer => parse_integer(value, type_name, primitive),
                                    SqlType::Text => parse_string(value, type_name, primitive),
                                }
                            })
                            .collect::<Result<Vec<_>>>()?;

                        Ok(Some(parse_filter(type_name, FilterValue::List(values))))
                    }
                    _ => Err(GqlError::new("Nested types are not supported")),
                })
            })
            .collect::<Result<Option<Vec<_>>>>()
    })
}

fn parse_integer(
    input: ValueAccessor<'_>,
    type_name: &str,
    primitive: Primitive,
) -> Result<FilterValue> {
    match primitive {
        Primitive::Bool(_) => input
            .boolean()
            .map(|b| FilterValue::Int(b as i64)) // treat bool as int per sqlite
            .map_err(|_| GqlError::new(format!("Expected boolean on field {}", type_name))),
        _ => input
            .i64()
            .map(FilterValue::Int)
            .map_err(|_| GqlError::new(format!("Expected integer on field {}", type_name))),
    }
}

fn parse_string(
    input: ValueAccessor<'_>,
    type_name: &str,
    primitive: Primitive,
) -> Result<FilterValue> {
    match input.string() {
        Ok(i) => match i.starts_with("0x") {
            true => Ok(FilterValue::String(format!("0x{:0>64}", i.strip_prefix("0x").unwrap()))), /* safe to unwrap since we know it starts with 0x */
            false => match primitive {
                // would overflow i128
                Primitive::U128(_) => match i.parse::<u128>() {
                    Ok(i) => Ok(FilterValue::String(format!("0x{:0>64x}", i))),
                    Err(_) => Ok(FilterValue::String(i.to_string())),
                },
                // signed and unsigned integers
                _ => match i.parse::<i128>() {
                    Ok(i) => Ok(FilterValue::String(format!("0x{:0>64x}", i))),
                    Err(_) => Ok(FilterValue::String(i.to_string())),
                },
            },
        },
        Err(_) => Err(GqlError::new(format!("Expected string on field {}", type_name))),
    }
}
