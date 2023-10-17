use std::str::FromStr;

use async_graphql::dynamic::{Field, InputObject, InputValue, ResolverContext, TypeRef};
use async_graphql::Name;
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
            .filter_map(|(type_name, type_data)| {
                // TODO: filter on nested and enum objects
                if type_data.is_nested() {
                    return None;
                } else if type_data.type_ref() == TypeRef::named("Enum") {
                    return Some(vec![(Name::new(type_name), type_data.clone())]);
                }

                let mut comparators = Comparator::iter()
                    .map(|comparator| {
                        let name = format!("{}{}", type_name, comparator.as_ref());
                        (Name::new(name), type_data.clone())
                    })
                    .collect::<Vec<_>>();

                comparators.push((Name::new(type_name), type_data.clone()));

                Some(comparators)
            })
            .flatten()
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
) -> Option<Vec<Filter>> {
    let where_input = ctx.args.get("where")?;
    let input_object = where_input.object().ok()?;

    where_mapping
        .iter()
        .filter_map(|(type_name, type_data)| {
            input_object.get(type_name).map(|input_filter| {
                let filter_value = match Primitive::from_str(&type_data.type_ref().to_string()) {
                    Ok(primitive) => match primitive.to_sql_type() {
                        SqlType::Integer => FilterValue::Int(input_filter.i64().ok()?),
                        SqlType::Text => {
                            FilterValue::String(input_filter.string().ok()?.to_string())
                        }
                    },
                    _ => FilterValue::String(input_filter.string().ok()?.to_string()),
                };

                Some(parse_filter(type_name, filter_value))
            })
        })
        .collect::<Option<Vec<_>>>()
}
