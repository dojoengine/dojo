pub mod connection;
pub mod entity;
pub mod event;
pub mod inputs;
pub mod model;
pub mod model_data;
pub mod system;
pub mod system_call;

use async_graphql::dynamic::{Enum, Field, FieldFuture, InputObject, Object, SubscriptionField};
use async_graphql::Value;

use self::connection::edge::EdgeObject;
use self::connection::ConnectionObject;
use crate::types::{TypeMapping, ValueMapping};

pub trait ObjectTrait: Send + Sync {
    // Name of the graphql object (eg "player")
    fn name(&self) -> &str;

    // Type name of the graphql object (eg "Player")
    fn type_name(&self) -> &str;

    // Type mapping defines the fields of the graphql object and their corresponding type
    fn type_mapping(&self) -> &TypeMapping;

    // Related field resolve to sibling graphql objects
    fn related_fields(&self) -> Option<Vec<Field>> {
        None
    }

    // Resolves single object queries, returns current object of type type_name (eg "Player")
    fn resolve_one(&self) -> Option<Field> {
        None
    }

    // Resolves plural object queries, returns type of {type_name}Connection (eg "PlayerConnection")
    fn resolve_many(&self) -> Option<Field> {
        None
    }

    // Resolves subscriptions, returns current object (eg "PlayerAdded")
    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        None
    }

    // Input objects consist of {type_name}WhereInput for filtering and {type_name}Order for
    // ordering
    fn input_objects(&self) -> Option<Vec<InputObject>> {
        None
    }

    // Enum objects
    fn enum_objects(&self) -> Option<Vec<Enum>> {
        None
    }

    // Connection type, if resolve_many is Some then register connection graphql obj, includes
    // {type_name}Connection and {type_name}Edge according to relay spec https://relay.dev/graphql/connections.htm
    fn connection(&self) -> Option<Vec<Object>> {
        self.resolve_many()?;

        let edge = EdgeObject::new(self.name().to_string(), self.type_name().to_string());
        let connection =
            ConnectionObject::new(self.name().to_string(), self.type_name().to_string());

        let mut objects = Vec::new();
        objects.extend(edge.objects());
        objects.extend(connection.objects());

        Some(objects)
    }

    fn objects(&self) -> Vec<Object> {
        let mut object = Object::new(self.type_name());

        for (field_name, type_data) in self.type_mapping().clone() {
            if type_data.is_nested() {
                continue;
            }

            let field = Field::new(field_name.to_string(), type_data.type_ref(), move |ctx| {
                let field_name = field_name.clone();

                FieldFuture::new(async move {
                    match ctx.parent_value.try_to_value()? {
                        Value::Object(values) => {
                            Ok(Some(values.get(&field_name).unwrap().clone())) // safe unwrap
                        }
                        _ => Err("incorrect value, requires Value::Object".into()),
                    }
                })
            });

            object = object.field(field);
        }

        // Add related graphql objects (eg event, system)
        if let Some(fields) = self.related_fields() {
            for field in fields {
                object = object.field(field);
            }
        }

        vec![object]
    }
}
