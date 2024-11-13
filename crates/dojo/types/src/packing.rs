use std::any::type_name;
use std::str::FromStr;

use crypto_bigint::U256;
use num_traits::ToPrimitive;
use starknet::core::types::{Felt, FromStrError};
use starknet::core::utils::{
    cairo_short_string_to_felt, parse_cairo_short_string, CairoShortStringToFeltError,
    ParseCairoShortStringError,
};

use crate::primitive::{Primitive, PrimitiveError};
use crate::schema::{self, EnumOption, Ty};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid schema: {0}")]
    InvalidSchema(String),
    #[error("Value out of range")]
    Primitive(#[from] PrimitiveError),
    #[error("Error when parsing felt: {0}")]
    FromStr(#[from] FromStrError),
    #[error(transparent)]
    ParseCairoShortStringError(#[from] ParseCairoShortStringError),
    #[error(transparent)]
    CairoShortStringToFeltError(#[from] CairoShortStringToFeltError),
}

impl ParseError {
    pub fn invalid_schema_with_msg(msg: &str) -> Self {
        Self::InvalidSchema(msg.to_string())
    }

    pub fn invalid_schema() -> Self {
        Self::InvalidSchema(String::from(""))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PackingError {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error("Error when unpacking entity")]
    UnpackingEntityError,
    #[error(transparent)]
    Primitive(#[from] PrimitiveError),
}

/// Unpacks a vector of packed values according to a given layout.
///
/// # Arguments
///
/// * `packed_values` - A vector of FieldElement values that are packed.
/// * `layout` - A vector of FieldElement values that describe the layout of the packed values.
///
/// # Returns
///
/// * `Result<Vec<Felt>, PackingError>` - A Result containing a vector of unpacked Felt values if
///   successful, or an error if unsuccessful.
pub fn unpack(mut packed: Vec<Felt>, layout: Vec<Felt>) -> Result<Vec<Felt>, PackingError> {
    packed.reverse();
    let mut unpacked = vec![];

    let felt = packed.pop().ok_or(PackingError::UnpackingEntityError)?;
    let mut unpacking = U256::from_be_slice(&felt.to_bytes_be());
    let mut offset = 0;
    // Iterate over the layout.
    for size in layout {
        let size: u8 = size.to_u8().ok_or_else(|| PrimitiveError::ValueOutOfRange {
            r#type: type_name::<u8>(),
            value: size,
        })?;

        let size: usize = size.into();
        let remaining_bits = 251 - offset;

        // If there are less remaining bits than the size, move to the next felt for unpacking.
        if remaining_bits < size {
            let felt = packed.pop().ok_or(PackingError::UnpackingEntityError)?;
            unpacking = U256::from_be_slice(&felt.to_bytes_be());
            offset = 0;
        }

        let mut mask = U256::from(0_u8);
        for _ in 0..size {
            mask = (mask << 1) | U256::from(1_u8);
        }

        let result = mask & (unpacking >> offset);
        let result_fe = Felt::from_hex(&result.to_string()).map_err(ParseError::FromStr)?;
        unpacked.push(result_fe);

        // Update unpacking to be the shifted value after extracting the result.
        offset += size;
    }

    Ok(unpacked)
}

/// Parse a raw schema of a model into a Cairo type, [Ty]
pub fn parse_ty(data: &[Felt]) -> Result<Ty, ParseError> {
    if data.is_empty() {
        return Err(ParseError::invalid_schema_with_msg(
            "The function parse_ty expects at least one felt to know the member type variant, \
             empty input found.",
        ));
    }

    let member_type: u8 = data[0].to_u8().ok_or_else(|| PrimitiveError::ValueOutOfRange {
        r#type: type_name::<u8>(),
        value: data[0],
    })?;

    match member_type {
        0 => parse_simple(&data[1..]),
        1 => parse_struct(&data[1..]),
        2 => parse_enum(&data[1..]),
        3 => parse_tuple(&data[1..]),
        4 => parse_array(&data[1..]),
        5 => parse_byte_array(),
        _ => Err(ParseError::invalid_schema_with_msg(&format!(
            "Unsupported member type variant `{}`.",
            member_type
        ))),
    }
}

fn parse_simple(data: &[Felt]) -> Result<Ty, ParseError> {
    let ty = parse_cairo_short_string(&data[0])?;
    let primitive = match Primitive::from_str(&ty) {
        Ok(primitive) => primitive,
        Err(_) => {
            return Err(ParseError::invalid_schema_with_msg(&format!(
                "Unsupported simple type primitive `{}`.",
                ty
            )));
        }
    };

    Ok(Ty::Primitive(primitive))
}

fn parse_struct(data: &[Felt]) -> Result<Ty, ParseError> {
    // A struct has at least 3 elements: name, attrs len, and children len.
    if data.len() < 3 {
        return Err(ParseError::invalid_schema_with_msg(&format!(
            "The function parse_struct expects at least three felts: name, attrs len, and \
             children len. Input of size {} found.",
            data.len()
        )));
    }

    let name = parse_cairo_short_string(&data[0])?;

    let attrs_len: u32 = data[1].to_u32().ok_or_else(|| PrimitiveError::ValueOutOfRange {
        r#type: type_name::<u32>(),
        value: data[1],
    })?;
    let attrs_slice_start = 2;
    let attrs_slice_end = attrs_slice_start + attrs_len as usize;
    let _attrs = &data[attrs_slice_start..attrs_slice_end];

    let children_len: u32 = data[attrs_slice_end].to_u32().ok_or_else(|| {
        PrimitiveError::ValueOutOfRange { r#type: type_name::<u32>(), value: data[attrs_slice_end] }
    })?;

    let children_len = children_len as usize;

    let mut children = vec![];
    let mut offset = attrs_slice_end + 1;

    for i in 0..children_len {
        let start = i + offset;
        let len: u32 = data[start].to_u32().ok_or_else(|| PrimitiveError::ValueOutOfRange {
            r#type: type_name::<u32>(),
            value: data[start],
        })?;
        let slice_start = start + 1;
        let slice_end = slice_start + len as usize;
        children.push(parse_member(&data[slice_start..slice_end])?);
        offset += len as usize;
    }

    Ok(Ty::Struct(schema::Struct { name, children }))
}

fn parse_member(data: &[Felt]) -> Result<schema::Member, ParseError> {
    if data.len() < 3 {
        return Err(ParseError::invalid_schema_with_msg(&format!(
            "The function parse_member expects at least three felts: name, attributes len, and \
             ty. Input of size {} found.",
            data.len()
        )));
    }

    let name = parse_cairo_short_string(&data[0])?;

    let attributes_len: u32 = data[1].to_u32().ok_or_else(|| PrimitiveError::ValueOutOfRange {
        r#type: type_name::<u32>(),
        value: data[1],
    })?;
    let slice_start = 2;
    let slice_end = slice_start + attributes_len as usize;
    let attributes = &data[slice_start..slice_end];

    let key = attributes.contains(&cairo_short_string_to_felt("key")?);
    let ty = parse_ty(&data[slice_end..])?;

    Ok(schema::Member { name, ty, key })
}

fn parse_enum(data: &[Felt]) -> Result<Ty, ParseError> {
    if data.len() < 3 {
        return Err(ParseError::invalid_schema_with_msg(&format!(
            "The function parse_enum expects at least three felts: name, attributes len, and \
             values len. Input of size {} found.",
            data.len()
        )));
    }

    let name = parse_cairo_short_string(&data[0])?;

    let attrs_len: u32 = data[1].to_u32().ok_or_else(|| PrimitiveError::ValueOutOfRange {
        r#type: type_name::<u32>(),
        value: data[1],
    })?;
    let attrs_slice_start = 2;
    let attrs_slice_end = attrs_slice_start + attrs_len as usize;
    let _attrs = &data[attrs_slice_start..attrs_slice_end];

    let values_len: u32 = data[attrs_slice_end].to_u32().ok_or_else(|| {
        PrimitiveError::ValueOutOfRange { r#type: type_name::<u32>(), value: data[attrs_slice_end] }
    })?;
    let values_len = values_len as usize;

    let mut values = vec![];
    let mut offset = attrs_slice_end + 1;

    for i in 0..values_len {
        let start = i + offset;
        let name = parse_cairo_short_string(&data[start])?;
        let slice_start = start + 2;
        let len: u32 = data[start + 3].to_u32().ok_or_else(|| PrimitiveError::ValueOutOfRange {
            r#type: type_name::<u32>(),
            value: data[start + 3],
        })?;
        let len = len + 1; // Account for Ty enum index

        let slice_end = slice_start + len as usize;
        values.push(EnumOption { name, ty: parse_ty(&data[slice_start..slice_end])? });
        offset += len as usize + 2;
    }

    Ok(Ty::Enum(schema::Enum { name, option: 0, options: values }))
}

fn parse_tuple(data: &[Felt]) -> Result<Ty, ParseError> {
    if data.is_empty() {
        // The unit type is defined as an empty tuple.
        return Ok(Ty::Tuple(vec![]));
    }

    let children_len: u32 = data[0].to_u32().ok_or_else(|| PrimitiveError::ValueOutOfRange {
        r#type: type_name::<u32>(),
        value: data[0],
    })?;
    let children_len = children_len as usize;

    let mut children = vec![];
    let mut offset = 1;

    for i in 0..children_len {
        let start = i + offset;
        let len: u32 = data[start].to_u32().ok_or_else(|| PrimitiveError::ValueOutOfRange {
            r#type: type_name::<u32>(),
            value: data[start],
        })?;
        let slice_start = start + 1;
        let slice_end = slice_start + len as usize;
        children.push(parse_ty(&data[slice_start..slice_end])?);
        offset += len as usize;
    }

    Ok(Ty::Tuple(children))
}

fn parse_array(data: &[Felt]) -> Result<Ty, ParseError> {
    if data.is_empty() || data.len() != 2 {
        return Err(ParseError::invalid_schema_with_msg(
            "The function parse_array expects exactly one felt to know the item type, empty input \
             found.",
        ));
    }

    // Arrays always have the same type for all elements.
    // In the introspect, the array type is given by the first (and unique) element in `Ty`.
    let mut v = data.to_vec();
    let _ = v.remove(0);

    let item_ty = parse_ty(v.as_slice())?;
    Ok(Ty::Array(vec![item_ty]))
}

fn parse_byte_array() -> Result<Ty, ParseError> {
    Ok(Ty::ByteArray("".to_string()))
}

#[cfg(test)]
mod tests {
    use starknet::core::types::Felt;
    use starknet::core::utils::cairo_short_string_to_felt;

    use super::*;

    #[test]
    fn parse_simple_with_invalid_value() {
        let data = [Felt::default()];
        assert!(parse_simple(&data).is_err());
    }

    #[test]
    fn parse_simple_with_valid_value() {
        let data = [cairo_short_string_to_felt("u8").unwrap()];
        assert_eq!(parse_simple(&data).unwrap(), Ty::Primitive(Primitive::U8(0)));
    }

    #[test]
    fn parse_struct_with_invalid_value() {
        // No attr len and no children.
        let data = [cairo_short_string_to_felt("bad_struct").unwrap()];
        assert!(parse_struct(&data).is_err());

        // Only with attr len.
        let data = [cairo_short_string_to_felt("bad_struct").unwrap(), Felt::default()];
        assert!(parse_struct(&data).is_err());
    }

    #[test]
    fn parse_struct_empty() {
        let data =
            [cairo_short_string_to_felt("empty_struct").unwrap(), Felt::default(), Felt::default()];

        assert_eq!(
            parse_struct(&data).unwrap(),
            Ty::Struct(schema::Struct { name: "empty_struct".to_string(), children: vec![] })
        );
    }

    #[test]
    fn parse_array_with_invalid_value() {
        let data = [Felt::default()];
        assert!(parse_array(&data).is_err());
    }
}
