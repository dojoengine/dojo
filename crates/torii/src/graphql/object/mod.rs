pub mod component;
pub mod component_state;
pub mod connection;
pub mod entity;
pub mod event;
mod query;
pub mod system;
pub mod system_call;

use async_graphql::dynamic::{Field, FieldFuture, Object, TypeRef};
use async_graphql::{Name, Value};
use indexmap::IndexMap;

// Type aliases for GraphQL fields
pub type TypeMapping = IndexMap<Name, String>;
pub type ValueMapping = IndexMap<Name, Value>;

pub trait ObjectTrait {
    // Name of the graphql object (eg "person")
    fn name(&self) -> &str;

    // Type name of the graphql object (eg "Person")
    fn type_name(&self) -> &str;

    // Type mapping defines the fields of the graphql object and their corresponding scalar type
    fn type_mapping(&self) -> &TypeMapping;

    // Resolves single object queries, returns current object of type type_name
    fn resolve_one(&self) -> Option<Field> {
        None
    }

    // Resolves plural object queries, returns type of {type_name}Connection (eg "PersonConnection")
    // https://relay.dev/graphql/connections.htm
    fn resolve_many(&self) -> Option<Field> {
        None
    }

    // Related graphql objects
    fn nested_fields(&self) -> Option<Vec<Field>> {
        None
    }

    // Create a new GraphQL object
    fn create(&self) -> Object {
        let mut object = Object::new(self.type_name());

        // Add fields (ie id, createdAt, etc) and their resolver
        for (field_name, field_type) in self.type_mapping() {
            let name = self.name().to_string();

            let field =
                Field::new(field_name.as_str(), TypeRef::named_nn(field_type), move |ctx| {
                    let name = name.clone();

                    FieldFuture::new(async move {
                        let mapping = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;

                        match mapping.get(name.as_str()) {
                            Some(value) => Ok(Some(value.clone())),
                            _ => Err(format!("{} field not found", name).into()),
                        }
                    })
                });

            object = object.field(field);
        }

        // Add related graphql objects (eg event, system)
        if let Some(nested_fields) = self.nested_fields() {
            for field in nested_fields {
                object = object.field(field);
            }
        }

        object
    }
}
