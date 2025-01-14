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
    pub nested_inputs: Vec<WhereInputObject>,
}

impl WhereInputObject {
    fn build_field_mapping(type_name: &str, type_data: &TypeData) -> Vec<(Name, TypeData)> {
        if type_data.type_ref() == TypeRef::named("Enum")
            || type_data.type_ref() == TypeRef::named("bool")
        {
            return vec![(Name::new(type_name), type_data.clone())];
        }

        Comparator::iter().fold(
            vec![(Name::new(type_name), type_data.clone())],
            |mut acc, comparator| {
                let name = format!("{}{}", type_name, comparator.as_ref());
                match comparator {
                    Comparator::In | Comparator::NotIn => {
                        acc.push((Name::new(name), TypeData::List(Box::new(type_data.clone()))))
                    }
                    _ => {
                        acc.push((Name::new(name), type_data.clone()));
                    }
                }
                acc
            },
        )
    }

    pub fn new(type_name: &str, object_types: &TypeMapping) -> Self {
        let mut nested_inputs = Vec::new();
        let mut where_mapping = TypeMapping::new();

        for (field_name, type_data) in object_types {
            if !type_data.is_list() {
                match type_data {
                    TypeData::Nested((_, nested_types)) => {
                        // Create nested input object
                        let nested_input = WhereInputObject::new(
                            &format!("{}_{}", type_name, field_name),
                            nested_types,
                        );

                        // Add field for the nested input using TypeData::Nested
                        where_mapping.insert(
                            Name::new(field_name),
                            TypeData::Nested((
                                TypeRef::named(&nested_input.type_name),
                                nested_types.clone(),
                            )),
                        );
                        nested_inputs.push(nested_input);
                    }
                    _ => {
                        // Add regular field with comparators
                        for (name, mapped_type) in Self::build_field_mapping(field_name, type_data)
                        {
                            where_mapping.insert(name, mapped_type);
                        }
                    }
                }
            }
        }

        Self {
            type_name: format!("{}WhereInput", type_name),
            type_mapping: where_mapping,
            nested_inputs,
        }
    }
}

impl WhereInputObject {
    pub fn input_objects(&self) -> Vec<InputObject> {
        let mut objects = vec![self.input_object()];
        for nested in &self.nested_inputs {
            objects.extend(nested.input_objects());
        }
        objects
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

fn parse_nested_where(
    input_object: &ValueAccessor<'_>,
    type_name: &str,
    type_data: &TypeData,
) -> Result<Vec<Filter>> {
    match type_data {
        TypeData::Nested((_, nested_mapping)) => {
            let nested_input = input_object.object()?;
            nested_mapping
                .iter()
                .filter_map(|(field_name, field_type)| {
                    nested_input.get(field_name).map(|input| {
                        let nested_filters = parse_where_value(
                            input,
                            &format!("{}.{}", type_name, field_name),
                            field_type,
                        )?;
                        Ok(nested_filters)
                    })
                })
                .collect::<Result<Vec<_>>>()
                .map(|filters| filters.into_iter().flatten().collect())
        }
        _ => Ok(vec![]),
    }
}

fn parse_where_value(
    input: ValueAccessor<'_>,
    field_path: &str,
    type_data: &TypeData,
) -> Result<Vec<Filter>> {
    match type_data {
        TypeData::Simple(_) => {
            if type_data.type_ref() == TypeRef::named("Enum") {
                let value = input.string()?;
                let mut filter =
                    parse_filter(&Name::new(field_path), FilterValue::String(value.to_string()));
                // complex enums have a nested option field for their variant name.
                // we trim the .option suffix to get the actual db field name
                filter.field = filter.field.trim_end_matches(".option").to_string();
                return Ok(vec![filter]);
            }

            let primitive = Primitive::from_str(&type_data.type_ref().to_string())?;
            let filter_value = match primitive.to_sql_type() {
                SqlType::Integer => parse_integer(input, field_path, primitive)?,
                SqlType::Text => parse_string(input, field_path, primitive)?,
            };

            Ok(vec![parse_filter(&Name::new(field_path), filter_value)])
        }
        TypeData::List(inner) => {
            let list = input.list()?;
            let values = list
                .iter()
                .map(|value| {
                    let primitive = Primitive::from_str(&inner.type_ref().to_string())?;
                    match primitive.to_sql_type() {
                        SqlType::Integer => parse_integer(value, field_path, primitive),
                        SqlType::Text => parse_string(value, field_path, primitive),
                    }
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(vec![parse_filter(&Name::new(field_path), FilterValue::List(values))])
        }
        TypeData::Nested(_) => parse_nested_where(&input, field_path, type_data),
    }
}

pub fn parse_where_argument(
    ctx: &ResolverContext<'_>,
    where_mapping: &TypeMapping,
) -> Result<Option<Vec<Filter>>> {
    ctx.args.get("where").map_or(Ok(None), |where_input| {
        let input_object = where_input.object()?;
        where_mapping
            .iter()
            .filter_map(|(field_name, type_data)| {
                input_object
                    .get(field_name)
                    .map(|input| parse_where_value(input, field_name, type_data))
            })
            .collect::<Result<Vec<_>>>()
            .map(|filters| Some(filters.into_iter().flatten().collect()))
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
