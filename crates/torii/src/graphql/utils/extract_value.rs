use std::borrow::Cow;

use async_graphql::Result;

use super::value_accessor::{ObjectAccessor, ValueAccessor};
use crate::graphql::ValueMapping;

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

pub fn extract<T: ExtractValue>(values: &ValueMapping, key: &str) -> Result<T> {
    let accessor = ObjectAccessor(Cow::Borrowed(values));
    let str = accessor.try_get(key)?;
    T::extract(str)
}
