pub mod component;
pub mod constants;
pub mod entity;
pub mod event;
pub mod schema;
pub mod server;
pub mod system;
pub mod system_call;

use async_graphql::dynamic::{Field, FieldFuture, Object, TypeRef};
use async_graphql::Value;
use indexmap::IndexMap;

pub type TypeMapping = IndexMap<String, String>;
pub type ValueMapping = IndexMap<String, Value>;

pub trait ObjectTraitStatic {
    fn new() -> Self;

    fn from(field_type_mapping: TypeMapping) -> Self;
}

pub trait ObjectTraitInstance {
    fn name(&self) -> &str;

    fn type_name(&self) -> &str;

    fn field_type_mapping(&self) -> &TypeMapping;

    fn field_resolvers(&self) -> Vec<Field>;

    // creates the graphql object based on the provided field type mapping
    fn create(&self) -> Object {
        (*self.field_type_mapping()).iter().fold(
            Object::new(self.type_name()),
            |obj, (field_name, field_type)| {
                let inner_name = field_name.clone();

                obj.field(Field::new(field_name, TypeRef::named_nn(field_type), move |ctx| {
                    let field_name = inner_name.clone();

                    FieldFuture::new(async move {
                        let mapping = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;
                        let value = mapping.get(field_name.as_str()).expect("field not found");
                        Ok(Some(value.clone()))
                    })
                }))
            },
        )
    }
}
