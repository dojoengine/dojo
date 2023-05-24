pub mod component;
pub mod constants;
pub mod entity;
pub mod entity_state;
pub mod event;
pub mod schema;
pub mod server;
pub mod system;
pub mod system_call;
pub mod types;

use async_graphql::dynamic::{Field, Object};
pub trait ObjectTrait {
    fn object() -> Object;
    fn resolvers() -> Vec<Field> {
        vec![]
    }
}
