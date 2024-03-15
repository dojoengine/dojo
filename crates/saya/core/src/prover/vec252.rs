use std::{ops::Deref, str::FromStr};

use cairo_felt::Felt252;
use serde::{de::Visitor, Deserialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VecFelt252Error {
    #[error("failed to parse number: {0}")]
    NumberParseError(#[from] std::num::ParseIntError),
    #[error("failed to parse bigint: {0}")]
    BigIntParseError(#[from] num_bigint::ParseBigIntError),
    #[error("number out of range")]
    NumberOutOfRange,
}

/// `VecFelt252` is a wrapper around a vector of `Arg`.
///
/// It provides convenience methods for working with a vector of `Arg` and implements
/// `Deref` to allow it to be treated like a vector of `Arg`.
#[derive(Debug, Clone)]
pub struct VecFelt252(Vec<Felt252>);

impl VecFelt252 {
    /// Creates a new `VecFelt252` from a vector of `Arg`.
    ///
    /// # Arguments
    ///
    /// * `args` - A vector of `Arg`.
    ///
    /// # Returns
    ///
    /// * `VecFelt252` - A new `VecFelt252` instance.
    #[must_use]
    pub fn new(args: Vec<Felt252>) -> Self {
        Self(args)
    }
}

impl Deref for VecFelt252 {
    type Target = Vec<Felt252>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<VecFelt252> for Vec<Felt252> {
    fn from(args: VecFelt252) -> Self {
        args.0
    }
}

impl From<Vec<Felt252>> for VecFelt252 {
    fn from(args: Vec<Felt252>) -> Self {
        Self(args)
    }
}

impl VecFelt252 {
    fn visit_seq_helper(seq: &[Value]) -> Result<Self, VecFelt252Error> {
        let iterator = seq.iter();
        let mut args = Vec::new();

        for arg in iterator {
            match arg {
                Value::Number(n) => {
                    let n = n.as_u64().ok_or(VecFelt252Error::NumberOutOfRange)?;
                    args.push(Felt252::from(n));
                }
                Value::String(n) => {
                    let n = num_bigint::BigUint::from_str(n)?;
                    args.push(Felt252::from_bytes_be(&n.to_bytes_be()));
                }
                Value::Array(a) => {
                    args.push(Felt252::from(a.len()));
                    let result = Self::visit_seq_helper(a)?;
                    args.extend(result.0);
                }
                _ => (),
            }
        }

        Ok(Self::new(args))
    }
}

impl<'de> Visitor<'de> for VecFelt252 {
    type Value = VecFelt252;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a list of arguments")
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut args = Vec::new();
        while let Some(arg) = seq.next_element()? {
            match arg {
                Value::Number(n) => args.push(Value::Number(n)),
                Value::String(n) => args.push(Value::String(n)),
                Value::Array(a) => args.push(Value::Array(a)),
                _ => return Err(serde::de::Error::custom("Invalid type")),
            }
        }

        Self::visit_seq_helper(&args).map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}

impl<'de> Deserialize<'de> for VecFelt252 {
    fn deserialize<D>(deserializer: D) -> Result<VecFelt252, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(VecFelt252(Vec::new()))
    }
}
