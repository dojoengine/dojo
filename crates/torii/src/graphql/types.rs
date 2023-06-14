use std::collections::HashSet;

pub struct ScalarType;

// Custom scalar types
impl ScalarType {
    pub const U8: &'static str = "u8";
    pub const U16: &'static str = "u16";
    pub const U32: &'static str = "u32";
    pub const U64: &'static str = "u64";
    pub const U128: &'static str = "u128";
    pub const U256: &'static str = "u256";
    pub const USIZE: &'static str = "usize";
    pub const BOOL: &'static str = "bool";
    pub const CURSOR: &'static str = "Cursor";
    pub const ADDRESS: &'static str = "Address";
    pub const DATE_TIME: &'static str = "DateTime";
    pub const FELT: &'static str = "FieldElement";

    pub fn types() -> HashSet<&'static str> {
        HashSet::from([
            ScalarType::U8,
            ScalarType::U16,
            ScalarType::U32,
            ScalarType::U64,
            ScalarType::U128,
            ScalarType::U256,
            ScalarType::USIZE,
            ScalarType::BOOL,
            ScalarType::CURSOR,
            ScalarType::ADDRESS,
            ScalarType::DATE_TIME,
            ScalarType::FELT,
        ])
    }
}
