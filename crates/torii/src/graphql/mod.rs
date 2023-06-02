pub mod component;
pub mod constants;
pub mod entity;
pub mod event;
pub mod schema;
pub mod server;
pub mod storage;
pub mod system;
pub mod system_call;

mod types;
mod utils;

use async_graphql::dynamic::{Field, FieldFuture, Object, TypeRef};
use async_graphql::{Name, Value};
use indexmap::IndexMap;

// Type aliases for GraphQL fields
pub type TypeMapping = IndexMap<Name, String>;
pub type ValueMapping = IndexMap<Name, Value>;

pub trait ObjectTrait {
    fn name(&self) -> &str;
    fn type_name(&self) -> &str;
    fn field_type_mapping(&self) -> &TypeMapping;
    fn field_resolvers(&self) -> Vec<Field>;
    fn related_fields(&self) -> Option<Vec<Field>> {
        None
    }

    // Create a new GraphQL object
    fn create(&self) -> Object {
        let mut object = Object::new(self.type_name());

        // Add fields (ie id, createdAt, etc)
        for (field_name, field_type) in self.field_type_mapping() {
            let field = create_field(field_name, field_type);
            object = object.field(field);
        }

        // Add related fields (ie event, system)
        if let Some(related_fields) = self.related_fields() {
            for field in related_fields {
                object = object.field(field);
            }
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
                _ => Err("field not found".into()),
            }
        })
    })
}
