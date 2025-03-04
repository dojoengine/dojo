use serde_json::Value;

pub mod query;
pub mod schema;

#[derive(Clone, Debug)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub fn get_tools() -> Vec<Tool> {
    vec![
        query::get_tool(),
        schema::get_tool(),
    ]
} 