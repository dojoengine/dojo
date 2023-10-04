use async_graphql::dynamic::TypeRef;
use async_graphql::{Name, Value};

use crate::object::ObjectTrait;
use crate::types::{TypeData, TypeMapping, ValueMapping};

pub struct ResultsObject {
    pub name: String,
    pub type_name: String,
    pub type_mapping: TypeMapping,
}

impl ResultsObject {
    pub fn new(name: String, type_name: String) -> Self {
        let type_mapping = TypeMapping::from([
            (Name::new("items"), TypeData::Simple(TypeRef::named_list(type_name.clone()))),
            (Name::new("totalCount"), TypeData::Simple(TypeRef::named_nn(TypeRef::INT))),
        ]);

        Self {
            name: format!("{}Results", name),
            type_name: format!("{}Results", type_name),
            type_mapping,
        }
    }
}

impl ObjectTrait for ResultsObject {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }
}

pub fn results_output(value_mapping: &[ValueMapping], total_count: i64) -> ValueMapping {
    let items: Vec<Value> = value_mapping.iter().map(|v| Value::Object(v.clone())).collect();

    ValueMapping::from([
        (Name::new("totalCount"), Value::from(total_count)),
        (Name::new("items"), Value::List(items)),
    ])
}
