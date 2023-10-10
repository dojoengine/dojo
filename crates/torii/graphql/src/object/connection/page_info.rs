use crate::mapping::PAGE_INFO_TYPE_MAPPING;
use crate::object::{ObjectTrait, TypeMapping};

pub struct PageInfoObject;

impl ObjectTrait for PageInfoObject {
    fn name(&self) -> (&str, &str) {
        ("pageInfo", "")
    }

    fn type_name(&self) -> &str {
        "PageInfo"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &PAGE_INFO_TYPE_MAPPING
    }
}
