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
    // Order matters here.
    // We want "NOT" comparators to be checked first
    // so that "NOT IN" is matched before "IN"
    NotIn,
    In,
    NotLike,
    Like,
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
            Comparator::In => write!(f, "IN"),
            Comparator::NotIn => write!(f, "NOT IN"),
            Comparator::Like => write!(f, "LIKE"),
            Comparator::NotLike => write!(f, "NOT LIKE"),
        }
    }
}

#[derive(Debug, Display)]
pub enum FilterValue {
    Int(i64),
    String(String),
    List(Vec<FilterValue>),
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
            return Filter {
                field: field.to_string(),
                comparator: comparator.clone(),
                value,
            };
        }
    }

    // If no suffix found assume equality comparison
    Filter { field: input.to_string(), comparator: Comparator::Eq, value }
}
