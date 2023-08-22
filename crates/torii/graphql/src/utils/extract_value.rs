use std::borrow::Cow;

use async_graphql::Result;

use super::value_accessor::{ObjectAccessor, ValueAccessor};
use crate::object::ValueMapping;

pub trait ExtractValue: Sized {
    fn extract(value_accessor: ValueAccessor<'_>) -> Result<Self>;
}

impl ExtractValue for i64 {
    fn extract(value_accessor: ValueAccessor<'_>) -> Result<Self> {
        value_accessor.i64()
    }
}

impl ExtractValue for String {
    fn extract(value_accessor: ValueAccessor<'_>) -> Result<Self> {
        let str = value_accessor.string()?;
        Ok(str.to_string())
    }
}

impl ExtractValue for Vec<String> {
    fn extract(value_accessor: ValueAccessor<'_>) -> Result<Vec<String>> {
        let str_array = value_accessor.list()?;
        let mut strings = Vec::new();
        for s in str_array.iter() {
            let string = s.string()?.to_string();
            strings.push(string);
        }
        Ok(strings)
    }
}

pub fn extract<T: ExtractValue>(values: &ValueMapping, key: &str) -> Result<T> {
    let accessor = ObjectAccessor(Cow::Borrowed(values));
    let value = accessor.try_get(key)?;
    T::extract(value)
}
