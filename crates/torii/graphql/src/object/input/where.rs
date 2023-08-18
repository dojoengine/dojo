use std::str::FromStr;

use async_graphql::dynamic::{Field, InputObject, InputValue, TypeRef};
use async_graphql::Name;

use crate::object::TypeMapping;
use crate::types::ScalarType;

pub struct WhereInputObject {}

impl WhereInputObject {
    // Iterate through an object's type mapping and create a new mapping for whereInput. For each of
    // the object type (component member), we add 6 additional types for comparators (great than,
    // not equal, etc). Only filtering on our custom scalar types and ignore async-graphql's types.
    // Due to sqlite column constraints, u8 thru u64 are treated as numerics and the rest are
    // treated as strings.
    pub fn new(type_name: &str, object_types: &TypeMapping) -> InputObject {
        let where_mapping: TypeMapping = object_types
            .iter()
            .filter_map(|(ty_name, ty)| {
                ScalarType::from_str(ty.to_string().as_str()).ok().map(|scalar_type| {
                    let ty =
                        if scalar_type.is_numeric_type() { TypeRef::INT } else { TypeRef::STRING };

                    let mut comparators = ["GT", "GTE", "LT", "LTE", "NEQ"]
                        .iter()
                        .map(|comparator| {
                            let name = format!("{}{}", ty_name, comparator);
                            (Name::new(name), TypeRef::named(ty))
                        })
                        .collect::<Vec<_>>();

                    comparators.push((Name::new(ty_name), TypeRef::named(ty)));

                    comparators
                })
            })
            .flatten()
            .collect();

        where_mapping.iter().fold(
            InputObject::new(format!("{}WhereInput", type_name)),
            |acc, (ty_name, ty)| {
                acc.field(InputValue::new(ty_name.to_string(), TypeRef::named(ty.to_string())))
            },
        )
    }
}

pub fn where_argument(field: Field, type_name: &str) -> Field {
    field.argument(InputValue::new("where", TypeRef::named(format!("{}WhereInput", type_name))))
}
