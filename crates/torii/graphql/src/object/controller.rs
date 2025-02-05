use async_graphql::dynamic::Field;

use super::{BasicObject, ResolvableObject, TypeMapping};
use crate::constants::{
    CONTROLLER_NAMES, CONTROLLER_TABLE, CONTROLLER_TYPE_NAME, ID_COLUMN,
};
use crate::mapping::CONTROLLER_MAPPING;
use crate::object::{resolve_many, resolve_one};

#[derive(Debug)]
pub struct ControllerObject;

impl BasicObject for ControllerObject {
    fn name(&self) -> (&str, &str) {
        CONTROLLER_NAMES
    }

    fn type_name(&self) -> &str {
        CONTROLLER_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &CONTROLLER_MAPPING
    }
}

impl ResolvableObject for ControllerObject {
    fn resolvers(&self) -> Vec<Field> {
        let resolve_one = resolve_one(
            CONTROLLER_TABLE,
            ID_COLUMN,
            self.name().0,
            self.type_name(),
            self.type_mapping(),
        );

        let resolve_many = resolve_many(
            CONTROLLER_TABLE,
            ID_COLUMN,
            self.name().1,
            self.type_name(),
            self.type_mapping(),
        );

        vec![resolve_one, resolve_many]
    }
} 