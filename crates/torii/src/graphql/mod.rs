pub mod component;
pub mod constants;
pub mod entity;
pub mod event;
pub mod schema;
pub mod server;
pub mod system;
pub mod system_call;

use async_graphql::dynamic::{Field, FieldFuture, Object, TypeRef};
use async_graphql::{Value, Name};
use indexmap::IndexMap;

// Type aliases for GraphQL fields
pub type TypeMapping = IndexMap<Name, &'static str>;
pub type ValueMapping = IndexMap<Name, Value>;

pub trait ObjectTraitStatic {
    fn new() -> Self;
    fn from(field_type_mapping: TypeMapping) -> Self;
}

pub trait ObjectTraitInstance {
    fn name(&self) -> &str;
    fn type_name(&self) -> &str;
    fn field_type_mapping(&self) -> &TypeMapping;
    fn field_resolvers(&self) -> Vec<Field>;

    // Create a new GraphQL object
    fn create(&self) -> Object {
        let mut object = Object::new(self.type_name());

        for (field_name, field_type) in self.field_type_mapping() {
            let field = create_field(field_name, field_type);
            object = object.field(field);
        }
        object
    }
}

fn create_field(name: &str, field_type: &str) -> Field {
    let outer_name = name.to_owned();

    Field::new(name, TypeRef::named_nn(field_type), move |ctx| {
        let inner_name = outer_name.to_owned();

        FieldFuture::new(async move {
            let mapping = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;

            match mapping.get(inner_name.as_str()) {
                Some(value) => Ok(Some(value.clone())),
                None => Ok(Some(Value::Null)),
            }
        })
    })
}
