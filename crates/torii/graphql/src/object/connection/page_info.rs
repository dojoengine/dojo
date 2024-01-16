use async_graphql::connection::PageInfo;
use async_graphql::dynamic::indexmap::IndexMap;
use async_graphql::{Name, Value};

use crate::constants::{PAGE_INFO_NAMES, PAGE_INFO_TYPE_NAME};
use crate::mapping::PAGE_INFO_TYPE_MAPPING;
use crate::object::{BasicObject, TypeMapping};

pub struct PageInfoObject;

impl BasicObject for PageInfoObject {
    fn name(&self) -> (&str, &str) {
        PAGE_INFO_NAMES
    }

    fn type_name(&self) -> &str {
        PAGE_INFO_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &PAGE_INFO_TYPE_MAPPING
    }
}

impl PageInfoObject {
    pub fn value(page_info: PageInfo) -> Value {
        Value::Object(IndexMap::from([
            (Name::new("hasPreviousPage"), Value::from(page_info.has_previous_page)),
            (Name::new("hasNextPage"), Value::from(page_info.has_next_page)),
            (
                Name::new("startCursor"),
                match page_info.start_cursor {
                    Some(val) => Value::from(val),
                    None => Value::Null,
                },
            ),
            (
                Name::new("endCursor"),
                match page_info.end_cursor {
                    Some(val) => Value::from(val),
                    None => Value::Null,
                },
            ),
        ]))
    }
}
