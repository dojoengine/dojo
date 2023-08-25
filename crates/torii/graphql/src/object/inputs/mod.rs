use async_graphql::dynamic::{Enum, InputObject};

use super::TypeMapping;

pub mod order_input;
pub mod where_input;

pub trait InputObjectTrait {
    // Type name of the input graphql object, we don't need a name as this will always be an input
    // object
    fn type_name(&self) -> &str;

    // Input fields and their corresponding type
    fn type_mapping(&self) -> &TypeMapping;

    // Create a new graphql input object with fields defined from type mapping
    fn input_object(&self) -> InputObject;

    // Enum objects
    fn enum_objects(&self) -> Option<Vec<Enum>> {
        None
    }
}
