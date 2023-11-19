use async_graphql::connection::PageInfo;
use async_graphql::dynamic::indexmap::IndexMap;
use async_graphql::dynamic::Field;
use async_graphql::{Name, Value};

use crate::mapping::PAGE_INFO_TYPE_MAPPING;
use crate::object::{ObjectTrait, TypeMapping};

pub struct PageInfoObject;

impl ObjectTrait for PageInfoObject {
    fn name(&self) -> (&str, &str) {
        ("pageInfo", "")
    }

    fn type_name(&self) -> &str {
        "World__PageInfo"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &PAGE_INFO_TYPE_MAPPING
    }

    fn resolve_one(&self) -> Option<Field> {
        None
    }

    fn resolve_many(&self) -> Option<Field> {
        None
    }
}

impl PageInfoObject {
    pub fn value(page_info: PageInfo) -> Value {
        Value::Object(IndexMap::from([
            (Name::new("has_previous_page"), Value::from(page_info.has_previous_page)),
            (Name::new("has_next_page"), Value::from(page_info.has_next_page)),
            (
                Name::new("start_cursor"),
                match page_info.start_cursor {
                    Some(val) => Value::from(val),
                    None => Value::Null,
                },
            ),
            (
                Name::new("end_cursor"),
                match page_info.end_cursor {
                    Some(val) => Value::from(val),
                    None => Value::Null,
                },
            ),
        ]))
    }
}
