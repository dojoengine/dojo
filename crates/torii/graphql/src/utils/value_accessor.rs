// Copied from async-graphql, useful helpers but was private to crate
// https://github.com/async-graphql/async-graphql/blob/master/src/dynamic/value_accessor.rs

use std::borrow::Cow;

use async_graphql::{Error, Name, Result, Value};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;

/// A value accessor
pub struct ValueAccessor<'a>(pub &'a Value);

impl<'a> ValueAccessor<'a> {
    /// Returns `true` if the value is null, otherwise returns `false`
    #[inline]
    #[allow(dead_code)]
    pub fn is_null(&self) -> bool {
        matches!(self.0, Value::Null)
    }

    /// Returns the boolean
    #[allow(dead_code)]
    pub fn boolean(&self) -> Result<bool> {
        match self.0 {
            Value::Boolean(b) => Ok(*b),
            _ => Err(Error::new("internal: not a boolean")),
        }
    }

    /// Returns the enum name
    #[allow(dead_code)]
    pub fn enum_name(&self) -> Result<&str> {
        match self.0 {
            Value::Enum(s) => Ok(s),
            Value::String(s) => Ok(s.as_str()),
            _ => Err(Error::new("internal: not an enum name")),
        }
    }

    /// Returns the number as `i64`
    #[allow(dead_code)]
    pub fn i64(&self) -> Result<i64> {
        if let Value::Number(number) = self.0 {
            if let Some(value) = number.as_i64() {
                return Ok(value);
            }
        }
        Err(Error::new("internal: not an signed integer"))
    }

    /// Returns the number as `u64`
    #[allow(dead_code)]
    pub fn u64(&self) -> Result<u64> {
        if let Value::Number(number) = self.0 {
            if let Some(value) = number.as_u64() {
                return Ok(value);
            }
        }
        Err(Error::new("internal: not an unsigned integer"))
    }

    /// Returns the number as `f32`
    #[allow(dead_code)]
    pub fn f32(&self) -> Result<f32> {
        if let Value::Number(number) = self.0 {
            if let Some(value) = number.as_f64() {
                return Ok(value as f32);
            }
        }
        Err(Error::new("internal: not a float"))
    }

    /// Returns the number as `f64`
    #[allow(dead_code)]
    pub fn f64(&self) -> Result<f64> {
        if let Value::Number(number) = self.0 {
            if let Some(value) = number.as_f64() {
                return Ok(value);
            }
        }
        Err(Error::new("internal: not a float"))
    }

    /// Returns the string value
    #[allow(dead_code)]
    pub fn string(&self) -> Result<&str> {
        if let Value::String(value) = self.0 {
            Ok(value)
        } else {
            Err(Error::new("internal: not a string"))
        }
    }

    /// Returns the object accessor
    #[allow(dead_code)]
    pub fn object(&self) -> Result<ObjectAccessor<'_>> {
        if let Value::Object(obj) = self.0 {
            Ok(ObjectAccessor(Cow::Borrowed(obj)))
        } else {
            Err(Error::new("internal: not an object"))
        }
    }

    /// Returns the list accessor
    #[allow(dead_code)]
    pub fn list(&self) -> Result<ListAccessor<'_>> {
        if let Value::List(list) = self.0 {
            Ok(ListAccessor(list))
        } else {
            Err(Error::new("internal: not a list"))
        }
    }

    /// Deserialize the value to `T`
    #[allow(dead_code)]
    pub fn deserialize<T: DeserializeOwned>(&self) -> Result<T> {
        T::deserialize(self.0.clone()).map_err(|err| format!("internal: {}", err).into())
    }
}

/// A object accessor
pub struct ObjectAccessor<'a>(pub(crate) Cow<'a, IndexMap<Name, Value>>);

impl<'a> ObjectAccessor<'a> {
    /// Return a reference to the value stored for `key`, if it is present,
    /// else `None`.
    #[inline]
    #[allow(dead_code)]
    pub fn get(&'a self, name: &str) -> Option<ValueAccessor<'a>> {
        self.0.get(name).map(ValueAccessor)
    }

    /// Like [`ObjectAccessor::get`], returns `Err` if the index does not exist
    #[inline]
    #[allow(dead_code)]
    pub fn try_get(&'a self, name: &str) -> Result<ValueAccessor<'a>> {
        self.0
            .get(name)
            .map(ValueAccessor)
            .ok_or_else(|| Error::new(format!("internal: key \"{}\" not found", name)))
    }

    /// Return an iterator over the key-value pairs of the object, in their
    /// order
    #[inline]
    #[allow(dead_code)]
    pub fn iter(&'a self) -> impl Iterator<Item = (&Name, ValueAccessor<'_>)> + 'a {
        self.0.iter().map(|(name, value)| (name, ValueAccessor(value)))
    }

    /// Return an iterator over the keys of the object, in their order
    #[inline]
    #[allow(dead_code)]
    pub fn keys(&'a self) -> impl Iterator<Item = &Name> + 'a {
        self.0.keys()
    }

    /// Return an iterator over the values of the object, in their order
    #[inline]
    #[allow(dead_code)]
    pub fn values(&'a self) -> impl Iterator<Item = ValueAccessor<'_>> + 'a {
        self.0.values().map(ValueAccessor)
    }

    /// Returns the number of elements in the object
    #[inline]
    #[allow(dead_code)]
    pub fn len(&'a self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the object has no members
    #[must_use]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A list accessor
pub struct ListAccessor<'a>(pub(crate) &'a [Value]);

impl<'a> ListAccessor<'a> {
    /// Returns the number of elements in the list
    #[inline]
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the list has a length of 0
    #[inline]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns an iterator over the list
    #[inline]
    #[allow(dead_code)]
    pub fn iter(&'a self) -> impl Iterator<Item = ValueAccessor<'_>> + 'a {
        self.0.iter().map(ValueAccessor)
    }

    /// Returns a reference to an element depending on the index
    #[inline]
    #[allow(dead_code)]
    pub fn get(&self, idx: usize) -> Option<ValueAccessor<'a>> {
        self.0.get(idx).map(ValueAccessor)
    }

    /// Like [`ListAccessor::get`], returns `Err` if the index does not exist
    #[inline]
    #[allow(dead_code)]
    pub fn try_get(&self, idx: usize) -> Result<ValueAccessor<'a>> {
        self.get(idx).ok_or_else(|| Error::new(format!("internal: index \"{}\" not found", idx)))
    }
}
