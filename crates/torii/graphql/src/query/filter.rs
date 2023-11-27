use core::fmt;

use async_graphql::Name;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter};

#[derive(AsRefStr, Debug, Clone, PartialEq, EnumIter)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Comparator {
    Gt,
    Gte,
    Lt,
    Lte,
    Neq,
    Eq,
}

impl fmt::Display for Comparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Comparator::Gt => write!(f, ">"),
            Comparator::Gte => write!(f, ">="),
            Comparator::Lt => write!(f, "<"),
            Comparator::Lte => write!(f, "<="),
            Comparator::Neq => write!(f, "!="),
            Comparator::Eq => write!(f, "="),
        }
    }
}

#[derive(Debug, Display)]
pub enum FilterValue {
    Int(i64),
    String(String),
}

#[derive(Debug)]
pub struct Filter {
    pub field: String,
    pub comparator: Comparator,
    pub value: FilterValue,
}

pub fn parse_filter(input: &Name, value: FilterValue) -> Filter {
    for comparator in Comparator::iter() {
        if let Some(field) = input.strip_suffix(comparator.as_ref()) {
            // Filtering only applies to model members which are stored in db with
            // external_{name}
            return Filter {
                field: format!("external_{}", field),
                comparator: comparator.clone(),
                value,
            };
        }
    }

    // If no suffix found assume equality comparison
    Filter { field: format!("external_{}", input), comparator: Comparator::Eq, value }
}
