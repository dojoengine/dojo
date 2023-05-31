pub struct ScalarType;

// Custom scalar types
impl ScalarType {
    pub const U8: &'static str = "U8";
    pub const U16: &'static str = "U16";
    pub const U32: &'static str = "U32";
    pub const U64: &'static str = "U64";
    pub const U128: &'static str = "U128";
    pub const U250: &'static str = "U250";
    pub const U256: &'static str = "U256";
    pub const CURSOR: &'static str = "Cursor";
    pub const ADDRESS: &'static str = "Address";
    pub const DATE_TIME: &'static str = "DateTime";
    pub const FELT: &'static str = "FieldElement";

    // NOTE: default types from async_graphql
    // TypeRef::ID
    // TypeRef::INT
    // TypeRef::FLOAT
    // TypeRef::STRING
    // TypeRef::BOOLEAN

    pub fn types() -> Vec<&'static str> {
        vec![
            ScalarType::U8,
            ScalarType::U16,
            ScalarType::U32,
            ScalarType::U64,
            ScalarType::U128,
            ScalarType::U250,
            ScalarType::U256,
            ScalarType::CURSOR,
            ScalarType::ADDRESS,
            ScalarType::DATE_TIME,
            ScalarType::FELT,
        ]
    }
}
