use anyhow::Result;
use async_graphql::dynamic::{
    Field, Object, Scalar, Schema, Subscription, SubscriptionField, Union,
};
use sqlx::SqlitePool;
use torii_core::types::Component;

use super::object::component_state::{type_mapping_query, ComponentStateObject};
use super::object::connection::page_info::PageInfoObject;
use super::object::entity::EntityObject;
use super::object::event::EventObject;
use super::object::system::SystemObject;
use super::object::system_call::SystemCallObject;
use super::object::ObjectTrait;
use super::types::ScalarType;
use super::utils::format_name;
use crate::object::component::ComponentObject;

// The graphql schema is built dynamically at runtime, this is because we won't know the schema of
// the components until runtime. There are however, predefined objects such as entities and
// system_calls, their schema is known but we generate them dynamically as well since async-graphql
// does not allow mixing of static and dynamic schemas.
pub async fn build_schema(pool: &SqlitePool) -> Result<Schema> {
    let mut schema_builder = Schema::build("Query", None, Some("Subscription"));

    // predefined objects
    let mut objects: Vec<Box<dyn ObjectTrait>> = vec![
        Box::<EntityObject>::default(),
        Box::<ComponentObject>::default(),
        Box::<SystemObject>::default(),
        Box::<EventObject>::default(),
        Box::<SystemCallObject>::default(),
        Box::<PageInfoObject>::default(),
    ];

    // register dynamic component objects
    let (component_objects, component_union) = component_objects(pool).await?;
    objects.extend(component_objects);

    schema_builder = schema_builder.register(component_union);

    // collect resolvers for single and plural queries
    let mut fields: Vec<Field> = Vec::new();
    for object in &objects {
        if let Some(resolve_one) = object.resolve_one() {
            fields.push(resolve_one);
        }
        if let Some(resolve_many) = object.resolve_many() {
            fields.push(resolve_many);
        }
    }

    // add field resolvers to query root
    let mut query_root = Object::new("Query");
    for field in fields {
        query_root = query_root.field(field);
    }

    // register custom scalars
    for scalar_type in ScalarType::types().iter() {
        schema_builder = schema_builder.register(Scalar::new(scalar_type.to_string()));
    }

    for object in &objects {
        // register enum objects
        if let Some(input_objects) = object.enum_objects() {
            for input in input_objects {
                schema_builder = schema_builder.register(input);
            }
        }

        // register input objects, whereInput and orderBy
        if let Some(input_objects) = object.input_objects() {
            for input in input_objects {
                schema_builder = schema_builder.register(input);
            }
        }

        // register connection types, relay
        if let Some(conn_objects) = object.connection() {
            for object in conn_objects {
                schema_builder = schema_builder.register(object);
            }
        }

        // register gql objects
        schema_builder = schema_builder.register(object.create());
    }

    // collect resolvers for single subscriptions
    let mut subscription_fields: Vec<SubscriptionField> = Vec::new();
    for object in &objects {
        if let Some(subscriptions) = object.subscriptions() {
            for sub in subscriptions {
                subscription_fields.push(sub);
            }
        }
    }

    // add field resolvers to subscription root
    let mut subscription_root = Subscription::new("Subscription");
    for field in subscription_fields {
        subscription_root = subscription_root.field(field);
    }

    schema_builder
        .register(query_root)
        .register(subscription_root)
        .data(pool.clone())
        .finish()
        .map_err(|e| e.into())
}

async fn component_objects(pool: &SqlitePool) -> Result<(Vec<Box<dyn ObjectTrait>>, Union)> {
    let mut conn = pool.acquire().await?;
    let mut objects: Vec<Box<dyn ObjectTrait>> = Vec::new();

    let components: Vec<Component> =
        sqlx::query_as("SELECT * FROM components").fetch_all(&mut conn).await?;

    // component union object
    let mut component_union = Union::new("ComponentUnion");

    // component state objects
    for component_metadata in components {
        let field_type_mapping = type_mapping_query(&mut conn, &component_metadata.id).await?;
        if !field_type_mapping.is_empty() {
            let (name, type_name) = format_name(&component_metadata.name);
            let state_object = Box::new(ComponentStateObject::new(
                name.clone(),
                type_name.clone(),
                field_type_mapping,
            ));

            component_union = component_union.possible_type(&type_name);
            objects.push(state_object);
        }
    }

    Ok((objects, component_union))
}
