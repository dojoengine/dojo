use anyhow::Result;
use async_graphql::dynamic::{Object, Scalar, Schema, Union};
use sqlx::SqlitePool;

use super::object::component::Component;
use super::object::component_state::{type_mapping_from, ComponentStateObject};
use super::object::entity::EntityObject;
use super::object::event::EventObject;
use super::object::system::SystemObject;
use super::object::system_call::SystemCallObject;
use super::object::ObjectTrait;
use super::types::ScalarType;
use super::utils::format_name;

pub async fn build_schema(pool: &SqlitePool) -> Result<Schema> {
    let mut schema_builder = Schema::build("Query", None, None);

    // static objects + dynamic objects
    let mut objects = static_objects();
    let (dynamic_objects, component_union) = dynamic_objects(pool).await?;
    objects.extend(dynamic_objects);

    // collect field resolvers
    let mut fields = Vec::new();
    for object in &objects {
        fields.extend(object.resolvers());
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

    // register gql objects
    for object in &objects {
        schema_builder = schema_builder.register(object.object());
    }

    // component union
    schema_builder = schema_builder.register(component_union);

    schema_builder.register(query_root).data(pool.clone()).finish().map_err(|e| e.into())
}

// predefined base objects
fn static_objects() -> Vec<Box<dyn ObjectTrait>> {
    vec![
        Box::new(EntityObject::new()),
        Box::new(SystemObject::new()),
        Box::new(EventObject::new()),
        Box::new(SystemCallObject::new()),
    ]
}

async fn dynamic_objects(pool: &SqlitePool) -> Result<(Vec<Box<dyn ObjectTrait>>, Union)> {
    let mut conn = pool.acquire().await?;
    let mut objects: Vec<Box<dyn ObjectTrait>> = Vec::new();

    let components: Vec<Component> =
        sqlx::query_as("SELECT * FROM components").fetch_all(&mut conn).await?;

    // component union object
    let mut component_union = Union::new("ComponentUnion");

    // component state objects
    for component_metadata in components {
        let field_type_mapping = type_mapping_from(&mut conn, &component_metadata.id).await?;
        let (name, type_name) = format_name(&component_metadata.name);

        let state_object = Box::new(ComponentStateObject::new(
            name.clone(),
            type_name.clone(),
            field_type_mapping,
        ));

        component_union = component_union.possible_type(&type_name);
        objects.push(state_object);
    }

    Ok((objects, component_union))
}
