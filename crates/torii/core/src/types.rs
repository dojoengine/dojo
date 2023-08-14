use core::fmt;

use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;

#[derive(Serialize, Deserialize)]
pub struct SQLFieldElement(pub FieldElement);

impl From<SQLFieldElement> for FieldElement {
    fn from(field_element: SQLFieldElement) -> Self {
        field_element.0
    }
}

impl TryFrom<String> for SQLFieldElement {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(SQLFieldElement(FieldElement::from_hex_be(&value)?))
    }
}

impl fmt::LowerHex for SQLFieldElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
