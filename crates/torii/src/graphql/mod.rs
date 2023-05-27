pub mod component;
pub mod constants;
pub mod entity;
pub mod event;
pub mod schema;
pub mod server;
pub mod system;
pub mod system_call;

use std::collections::HashMap;

use async_graphql::dynamic::{Field, FieldFuture, Object, TypeRef};
use async_graphql::Value;

pub type FieldTypeMapping = HashMap<String, String>;
pub type FieldValueMapping = HashMap<String, Value>;

pub trait ObjectTrait {
    fn new(field_type_mappings: FieldTypeMapping) -> Self;

    fn name(&self) -> &str;

    fn type_name(&self) -> &str;

    fn field_type_mappings(&self) -> &FieldTypeMapping;

    // creates the graphql object based on the provided field type mapping
    fn object(&self) -> Object {
        (*self.field_type_mappings()).iter().fold(
            Object::new(self.type_name()),
            |obj, (field_name, field_type)| {
                let inner_name = field_name.clone();

                obj.field(Field::new(field_name, TypeRef::named_nn(field_type), move |ctx| {
                    let field_name = inner_name.clone();

                    FieldFuture::new(async move {
                        let mapping = ctx.parent_value.try_downcast_ref::<FieldValueMapping>()?;
                        let value = mapping.get(field_name.as_str()).expect("field not found");
                        Ok(Some(value.clone()))
                    })
                }))
            },
        )
    }

    fn field_resolvers(&self) -> Vec<Field>;
}
