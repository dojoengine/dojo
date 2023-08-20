use async_graphql::dynamic::{Enum, Field, InputObject, InputValue, TypeRef};

use super::InputObjectTrait;
use crate::object::TypeMapping;

pub struct OrderInputObject {
    pub type_name: String,
    pub type_mapping: TypeMapping,
}

impl OrderInputObject {
    pub fn new(type_name: &str, object_types: &TypeMapping) -> Self {
        Self { type_name: format!("{}Order", type_name), type_mapping: object_types.clone() }
    }
}

impl InputObjectTrait for OrderInputObject {
    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }

    fn input_object(&self) -> InputObject {
        InputObject::new(self.type_name())
            .field(InputValue::new("direction", TypeRef::named_nn("Direction")))
            .field(InputValue::new(
                "field",
                TypeRef::named_nn(format!("{}OrderField", self.type_name())),
            ))
    }

    fn enum_objects(&self) -> Option<Vec<Enum>> {
        // Direction enum has only two members ASC and DESC
        let direction = Enum::new("Direction").item("ASC").item("DESC");

        // Field Order enum consist of all members of a component
        let field_order = self
            .type_mapping
            .iter()
            .fold(Enum::new(format!("{}OrderField", self.type_name())), |acc, (ty_name, _)| {
                acc.item(ty_name.to_ascii_uppercase())
            });

        Some(vec![direction, field_order])
    }
}

pub fn order_argument(field: Field, type_name: &str) -> Field {
    field.argument(InputValue::new("order", TypeRef::named(format!("{}Order", type_name))))
}
