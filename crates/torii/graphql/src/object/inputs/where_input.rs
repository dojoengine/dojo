use std::str::FromStr;

use async_graphql::dynamic::{Field, InputObject, InputValue, ResolverContext, TypeRef};
use async_graphql::{Error, Name};
use dojo_types::primitive::Primitive;

use super::InputObjectTrait;
use crate::object::TypeMapping;
use crate::query::filter::{parse_filter, Filter, FilterValue};

pub struct WhereInputObject {
    pub type_name: String,
    pub type_mapping: TypeMapping,
}

impl WhereInputObject {
    // Iterate through an object's type mapping and create a new mapping for whereInput. For each of
    // the object type (model member), we add 6 additional types for comparators (great than,
    // not equal, etc). Only filter on our custom scalar types and ignore async-graphql's types.
    // Due to sqlite column constraints, u8 thru u64 are treated as numerics and the rest of the
    // types are treated as strings.
    pub fn new(type_name: &str, object_types: &TypeMapping) -> Self {
        let where_mapping = object_types
            .iter()
            .filter_map(|(type_name, type_data)| {
                // TODO: filter on nested objects
                if type_data.is_nested() {
                    return None;
                }

                let mut comparators = ["GT", "GTE", "LT", "LTE", "NEQ"]
                    .iter()
                    .map(|comparator| {
                        let name = format!("{}{}", type_name, comparator);
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
) -> Result<Vec<Filter>, Error> {
    let where_input = match ctx.args.try_get("where") {
        Ok(input) => input,
        Err(_) => return Ok(vec![]),
    };

    let input_object = where_input.object()?;
    where_mapping
        .iter()
        .filter_map(|(type_name, type_data)| {
            input_object.get(type_name).map(|input_filter| {
                let primitive = Primitive::from_str(&type_data.type_ref().to_string())?;
                let data = match primitive.to_sql_type().as_str() {
                    "TEXT" => FilterValue::String(input_filter.string()?.to_string()),
                    "INTEGER" => FilterValue::Int(input_filter.i64()?),
                    _ => return Err(Error::from("Unsupported `where` argument type")),
                };

                Ok(parse_filter(type_name, data))
            })
        })
        .collect::<Result<Vec<_>, _>>()
}
