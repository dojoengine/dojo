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

pub struct WhereInputObject {
    pub type_name: String,
    pub type_mapping: TypeMapping,
}

impl WhereInputObject {
    // Iterate through an object's type mapping and create a new mapping for whereInput. For each of
    // the object type (model member), we add 6 additional types for comparators (great than,
    // not equal, etc)
    pub fn new(type_name: &str, object_types: &TypeMapping) -> Self {
        let where_mapping = object_types
            .iter()
            .filter(|(_, type_data)| !type_data.is_nested())
            .flat_map(|(type_name, type_data)| {
                // TODO: filter on nested and enum objects
                if type_data.type_ref() == TypeRef::named("Enum")
                    || type_data.type_ref() == TypeRef::named("bool")
                {
                    return vec![(Name::new(type_name), type_data.clone())];
                }

                Comparator::iter().fold(
                    vec![(Name::new(type_name), type_data.clone())],
                    |mut acc, comparator| {
                        let name = format!("{}{}", type_name, comparator.as_ref());
                        acc.push((Name::new(name), type_data.clone()));

                        acc
                    },
                )
            })
            .collect();

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
                input_object.get(type_name).map(|input| {
                    let primitive = Primitive::from_str(&type_data.type_ref().to_string())?;
                    let filter_value = match primitive.to_sql_type() {
                        SqlType::Integer => parse_integer(input, type_name, primitive)?,
                        SqlType::Text => parse_string(input, type_name)?,
                    };

                    Ok(Some(parse_filter(type_name, filter_value)))
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

fn parse_string(input: ValueAccessor<'_>, type_name: &str) -> Result<FilterValue> {
    input
        .string()
        .map(|i| FilterValue::String(i.to_string()))
        .map_err(|_| GqlError::new(format!("Expected string on field {}", type_name)))
}
