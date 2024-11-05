use anyhow::{self, Result};
use cainome::cairo_serde::{ByteArray, CairoSerde};
use num_bigint::BigUint;
use starknet::core::types::{Felt, FromStrError};
use starknet::core::utils::cairo_short_string_to_felt;

/// An error that occurs while decoding calldata.
#[derive(thiserror::Error, Debug)]
pub enum CalldataDecoderError {
    #[error("Parse Error: {0}")]
    ParseError(String),
    #[error(transparent)]
    FromStr(#[from] FromStrError),
    #[error(transparent)]
    CairoSerde(#[from] cainome::cairo_serde::Error),
    #[error(transparent)]
    Hex(#[from] hex::FromHexError),
    #[error(transparent)]
    FromStrInt(#[from] std::num::ParseIntError),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] starknet::core::utils::CairoShortStringToFeltError),
}

pub type DecoderResult<T, E = CalldataDecoderError> = Result<T, E>;

const ITEM_DELIMITER: char = ',';
const ITEM_PREFIX_DELIMITER: char = ':';

/// A trait for decoding calldata into a vector of Felts.
trait CalldataDecoder {
    fn decode(&self, input: &str) -> DecoderResult<Vec<Felt>>;
}

/// Decodes a u256 string into a [`Felt`]s array representing
/// a u256 value split into two 128-bit words.
struct U256CalldataDecoder;
impl CalldataDecoder for U256CalldataDecoder {
    fn decode(&self, input: &str) -> DecoderResult<Vec<Felt>> {
        let bigint = if let Some(hex_str) = input.strip_prefix("0x") {
            let unsigned_bytes = if hex_str.len() % 2 == 0 {
                hex::decode(hex_str)?
            } else {
                let mut padded = String::from("0");
                padded.push_str(hex_str);
                hex::decode(&padded)?
            };

            BigUint::from_bytes_be(&unsigned_bytes)
        } else {
            // Assuming decimal.
            let digits = input
                .chars()
                .map(|c| c.to_string().parse::<u8>())
                .collect::<std::result::Result<Vec<_>, _>>()?;

            // All elements in `digits` must be less than 10 so this is safe
            BigUint::from_radix_be(&digits, 10).unwrap()
        };

        let u128_max_plus_1 =
            BigUint::from_bytes_be(&hex_literal::hex!("0100000000000000000000000000000000"));

        let high = &bigint / &u128_max_plus_1;
        if high >= u128_max_plus_1 {
            return Err(CalldataDecoderError::ParseError("u256 value out of range".to_string()));
        }

        let low = &bigint % &u128_max_plus_1;

        // Unwrapping is safe as these are never out of range
        let high = Felt::from_bytes_be_slice(&high.to_bytes_be());
        let low = Felt::from_bytes_be_slice(&low.to_bytes_be());

        Ok(vec![low, high])
    }
}

/// Decodes a string (ByteArray) into a [`Felt`]s array representing
/// a serialized Cairo ByteArray.
struct StrCalldataDecoder;
impl CalldataDecoder for StrCalldataDecoder {
    fn decode(&self, input: &str) -> DecoderResult<Vec<Felt>> {
        let ba = ByteArray::from_string(input)?;
        Ok(ByteArray::cairo_serialize(&ba))
    }
}

/// Decodes a cairo short string into a [`Felt`].
struct ShortStrCalldataDecoder;
impl CalldataDecoder for ShortStrCalldataDecoder {
    fn decode(&self, input: &str) -> DecoderResult<Vec<Felt>> {
        Ok(vec![cairo_short_string_to_felt(input)?])
    }
}

/// Decodes a signed integer into a [`Felt`]
struct SignedIntegerCalldataDecoder;
impl CalldataDecoder for SignedIntegerCalldataDecoder {
    fn decode(&self, input: &str) -> DecoderResult<Vec<Felt>> {
        if let Ok(value) = input.parse::<i128>() {
            Ok(vec![value.into()])
        } else {
            Err(CalldataDecoderError::ParseError("Invalid numeric string".to_string()))
        }
    }
}

/// Decodes a string into a [`Felt`], either from hexadecimal or decimal string.
struct DefaultCalldataDecoder;
impl CalldataDecoder for DefaultCalldataDecoder {
    fn decode(&self, input: &str) -> DecoderResult<Vec<Felt>> {
        let felt = if let Some(hex_str) = input.strip_prefix("0x") {
            Felt::from_hex(hex_str)?
        } else {
            Felt::from_dec_str(input)?
        };

        Ok(vec![felt])
    }
}

/// Decodes a string of calldata items into a vector of Felts.
///
/// # Arguments:
///
/// * `input` - The input string to decode, with each item separated by a comma. Inputs can have
///   prefixes to indicate the type of the item.
///
/// # Returns
/// A vector of [`Felt`]s.
///
/// # Example
///
/// ```
/// let input = "u256:0x1,str:hello,64";
/// let result = decode_calldata(input).unwrap();
/// ```
pub fn decode_calldata(input: &str) -> DecoderResult<Vec<Felt>> {
    let items = input.split(ITEM_DELIMITER);
    let mut calldata = vec![];

    for item in items {
        calldata.extend(decode_inner(item)?);
    }

    Ok(calldata)
}

/// Decodes a single item of calldata into a vector of Felts.
///
/// # Arguments
///
/// * `item` - The item to decode.
///
/// # Returns
/// A vector of [`Felt`]s.
fn decode_inner(item: &str) -> DecoderResult<Vec<Felt>> {
    let item = item.trim();

    let felts = if let Some((prefix, value)) = item.split_once(ITEM_PREFIX_DELIMITER) {
        match prefix {
            "u256" => U256CalldataDecoder.decode(value)?,
            "str" => StrCalldataDecoder.decode(value)?,
            "sstr" => ShortStrCalldataDecoder.decode(value)?,
            "int" => SignedIntegerCalldataDecoder.decode(value)?,
            _ => DefaultCalldataDecoder.decode(item)?,
        }
    } else {
        DefaultCalldataDecoder.decode(item)?
    };

    Ok(felts)
}

#[cfg(test)]
mod tests {
    use starknet::core::utils::cairo_short_string_to_felt;

    use super::*;

    #[test]
    fn test_u256_decoder_hex() {
        let input = "u256:0x1";
        let expected = vec![Felt::ONE, Felt::ZERO];
        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_u256_decoder_decimal() {
        let input = "u256:12";
        let expected = vec![12_u128.into(), 0_u128.into()];

        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_short_str_decoder() {
        let input = "sstr:hello";
        let expected = vec![cairo_short_string_to_felt("hello").unwrap()];

        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_str_decoder() {
        let input = "str:hello";
        let expected =
            vec![0_u128.into(), cairo_short_string_to_felt("hello").unwrap(), 5_u128.into()];

        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_str_decoder_long() {
        let input = "str:hello with spaces and a long string longer than 31 chars";

        let expected = vec![
            // Length of the data.
            1_u128.into(),
            // Data element.
            cairo_short_string_to_felt("hello with spaces and a long st").unwrap(),
            // Remaining word.
            cairo_short_string_to_felt("ring longer than 31 chars").unwrap(),
            // Remaining word's length.
            25_u128.into(),
        ];

        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_default_decoder_hex() {
        let input = "0x64";
        let expected = vec![100_u128.into()];
        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_default_decoder_decimal() {
        let input = "64";
        let expected = vec![64_u128.into()];
        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_signed_integer_decoder_i8() {
        let input = "-64";
        let signed_i8: i8 = -64;
        let expected = vec![signed_i8.into()];
        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_signed_integer_decoder_i16() {
        let input = "-12345";
        let signed_i16: i16 = -12345;
        let expected = vec![signed_i16.into()];
        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_signed_integer_decoder_i32() {
        let input = "-987654321";
        let signed_i32: i32 = -987654321;
        let expected = vec![signed_i32.into()];
        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_signed_integer_decoder_i64() {
        let input = "-1234567890123456789";
        let signed_i64: i64 = -1234567890123456789;
        let expected = vec![signed_i64.into()];
        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_signed_integer_decoder_i128() {
        let input = "-123456789012345678901234567890123456";
        let signed_i128: i128 = -123456789012345678901234567890123456;
        let expected = vec![signed_i128.into()];
        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_combined_decoders() {
        let input = "u256:0x64,str:world,987654,0x123";
        let expected = vec![
            // U256 low.
            100_u128.into(),
            // U256 high.
            0_u128.into(),
            // Str data len.
            0_u128.into(),
            // Str pending word.
            cairo_short_string_to_felt("world").unwrap(),
            // Str pending word len.
            5_u128.into(),
            // Decimal value.
            987654_u128.into(),
            // Hex value.
            291_u128.into(),
        ];

        let result = decode_calldata(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_invalid_signed_integer_decoder() {
        let input = "-12345abc";
        let decoder = SignedIntegerCalldataDecoder;
        let result = decoder.decode(input);
        assert!(result.is_err());
    }
}
