use anyhow::Result;
use async_graphql::dynamic::{Object, Scalar, Schema};
use sqlx::SqlitePool;

use super::object::component::{Component, ComponentObject};
use super::object::entity::EntityObject;
use super::object::event::EventObject;
use super::object::storage::{type_mapping_from_definition, StorageObject};
use super::object::system::SystemObject;
use super::object::system_call::SystemCallObject;
use super::object::ObjectTrait;
use super::types::ScalarType;
use super::utils::format_name;

pub async fn build_schema(pool: &SqlitePool) -> Result<Schema> {
    let mut schema_builder = Schema::build("Query", None, None);

    // static objects + dynamic objects (component and storage objects)
    let mut objects = static_objects();
    objects.extend(dynamic_objects(pool).await?);

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
        schema_builder = schema_builder.register(Scalar::new(*scalar_type));
    }

    // register gql objects and union
    for object in &objects {
        schema_builder = schema_builder.register(object.object());
        if let Some(unions) = object.unions() {
            for union in unions {
                schema_builder = schema_builder.register(union);
            }
        }
    }

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

async fn dynamic_objects(pool: &SqlitePool) -> Result<Vec<Box<dyn ObjectTrait>>> {
    let mut conn = pool.acquire().await?;
    let mut objects = Vec::new();

    // storage objects
    let components: Vec<Component> =
        sqlx::query_as("SELECT * FROM components").fetch_all(&mut conn).await?;
    for component in components {
        let storage_object = process_component(component)?;
        objects.push(storage_object);
    }

    let storage_names: Vec<String> = objects.iter().map(|obj| obj.name().to_string()).collect();

    // component object
    let component = ComponentObject::new(storage_names);
    objects.push(Box::new(component));

    Ok(objects)
}

fn process_component(component: Component) -> Result<Box<dyn ObjectTrait>> {
    let field_type_mapping = type_mapping_from_definition(&component.storage_definition)?;
    let (name, type_name) = format_name(component.name.as_str());
    Ok(Box::new(StorageObject::new(name, type_name, field_type_mapping)))
}
