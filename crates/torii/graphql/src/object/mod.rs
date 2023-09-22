pub mod connection;
pub mod entity;
pub mod event;
pub mod inputs;
pub mod model;
pub mod model_state;
pub mod system;
pub mod system_call;

use async_graphql::dynamic::{
    Enum, Field, FieldFuture, InputObject, Object, SubscriptionField, TypeRef,
};
use async_graphql::{Error, Name, Value};
use indexmap::IndexMap;

use self::connection::edge::EdgeObject;
use self::connection::ConnectionObject;

// Type aliases for GraphQL fields
pub type TypeMapping = IndexMap<Name, TypeRef>;
pub type ValueMapping = IndexMap<Name, Value>;

pub trait ObjectTrait {
    // Name of the graphql object (eg "player")
    fn name(&self) -> &str;

    // Type name of the graphql object (eg "Player")
    fn type_name(&self) -> &str;

    // Type mapping defines the fields of the graphql object and their corresponding type
    fn type_mapping(&self) -> &TypeMapping;

    // Related graphql objects
    fn nested_fields(&self) -> Option<Vec<Field>> {
        None
    }

    // Resolves single object queries, returns current object of type type_name (eg "Player")
    fn resolve_one(&self) -> Option<Field> {
        None
    }

    // Resolves subscriptions, returns current object (eg "PlayerAdded")
    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        None
    }
    // Resolves plural object queries, returns type of {type_name}Connection (eg "PlayerConnection")
    fn resolve_many(&self) -> Option<Field> {
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

        Some(vec![edge.create(), connection.create()])
    }

    // Create a new graphql object and also define its fields from type mapping
    fn create(&self) -> Object {
        let mut object = Object::new(self.type_name());

        for (field_name, field_type) in self.type_mapping() {
            let field_name = field_name.clone();
            let field_type = field_type.clone();

            let field = Field::new(field_name.to_string(), field_type, move |ctx| {
                let field_name = field_name.clone();

                FieldFuture::new(async move {
                    // All direct queries, single and plural, passes down results as Value of type
                    // Object, and Object is an indexmap that contains fields
                    // and their corresponding result. The result can also be
                    // another Object. This is evaluated repeatedly until Value is a string or
                    // number.
                    if let Some(value) = ctx.parent_value.as_value() {
                        return match value {
                            Value::Object(indexmap) => field_value(indexmap, field_name.as_str()),
                            _ => Err("Incorrect value, requires Value::Object".into()),
                        };
                    }

                    // Component union queries is a special case, it instead passes down a
                    // IndexMap<Name, Value>. This could be avoided if
                    // async-graphql allowed union resolver to be passed down as Value.
                    if let Some(indexmap) = ctx.parent_value.downcast_ref::<ValueMapping>() {
                        return field_value(indexmap, field_name.as_str());
                    }

                    Err("Field resolver only accepts Value or IndexMap".into())
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

fn field_value(value_mapping: &ValueMapping, field_name: &str) -> Result<Option<Value>, Error> {
    match value_mapping.get(field_name) {
        Some(value) => Ok(Some(value.clone())),
        _ => Err(format!("{} field not found", field_name).into()),
    }
}
