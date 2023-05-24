use std::collections::HashMap;

use async_graphql::dynamic::{
    Field, FieldFuture, Object, Scalar, Schema, SchemaBuilder, SchemaError, TypeRef,
};
use async_graphql::Value;
use sqlx::SqlitePool;

use super::component::Component;
use super::entity::Entity;
use super::entity_state::EntityState;
use super::event::Event;
use super::system::System;
use super::system_call::SystemCall;
use super::types::SCALAR_TYPES;
use super::ObjectTrait;

pub fn build_schema(pool: &SqlitePool) -> Result<Schema, SchemaError> {
    let mut fields = Entity::resolvers();
    fields.extend(Component::resolvers());
    fields.extend(System::resolvers());
    fields.extend(Event::resolvers());

    let query_root =
        fields.into_iter().fold(Object::new("Query"), |query_root, field| query_root.field(field));

    // register custom scalars
    let mut schema_builder: SchemaBuilder = SCALAR_TYPES.iter().fold(
        Schema::build("Query", None, None),
        |schema_builder, scalar_type| {
            if *scalar_type == "Boolean" || *scalar_type == "ID" {
                schema_builder
            } else {
                schema_builder.register(Scalar::new(*scalar_type))
            }
        },
    );

    // register default gql objects
    schema_builder = schema_builder
        .register(Entity::object())
        .register(Component::object())
        .register(System::object())
        .register(Event::object())
        .register(EntityState::object())
        .register(SystemCall::object())
        .register(query_root)
        .data(pool.clone());

    // TODO: dynamically create component objects from schema table and register them

    schema_builder.finish()
}

fn _create_dynamic_object(
    type_name: &'static str,
    fields: &HashMap<&'static str, &'static str>,
) -> Object {
    let object = fields.iter().fold(Object::new(type_name), |object, (field_name, field_type)| {
        let field = Field::new(*field_name, TypeRef::named_nn(*field_type), |ctx| {
            let data = ctx.parent_value.try_downcast_ref::<HashMap<String, String>>().unwrap();
            let field_value = data.get(*field_name).unwrap();
            FieldFuture::new(async move { Ok(Some(Value::from(field_value.as_str()))) })
        });
        object.field(field)
    });

    object
}
