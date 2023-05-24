use std::collections::HashSet;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref SCALAR_TYPES: HashSet<&'static str> = HashSet::from([
        "U8",
        "U16",
        "U32",
        "U64",
        "U128",
        "U250",
        "U256",
        "Cursor",
        "Boolean",
        "Address",
        "DateTime",
        "FieldElement",
    ]);
    pub static ref NUMERIC_SCALAR_TYPES: HashSet<&'static str> =
        HashSet::from(["U8", "U16", "U32", "U64", "U128", "U250", "U256",]);
    pub static ref STRING_SCALAR_TYPES: HashSet<&'static str> =
        HashSet::from(["Cursor", "Address", "DateTime", "FieldElement",]);
}
