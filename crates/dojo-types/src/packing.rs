use std::str::FromStr;

use crypto_bigint::U256;
use starknet::core::types::{FieldElement, FromStrError, ValueOutOfRangeError};
use starknet::core::utils::{
    cairo_short_string_to_felt, parse_cairo_short_string, CairoShortStringToFeltError,
    ParseCairoShortStringError,
};

use crate::primitive::Primitive;
use crate::schema::{self, Ty};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid schema")]
    InvalidSchema,
    #[error("Error when parsing felt: {0}")]
    ValueOutOfRange(#[from] ValueOutOfRangeError),
    #[error("Error when parsing felt: {0}")]
    FromStr(#[from] FromStrError),
    #[error(transparent)]
    ParseCairoShortStringError(#[from] ParseCairoShortStringError),
    #[error(transparent)]
    CairoShortStringToFeltError(#[from] CairoShortStringToFeltError),
}

#[derive(Debug, thiserror::Error)]
pub enum PackingError {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error("Error when unpacking entity")]
    UnpackingEntityError,
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
/// * `Result<Vec<FieldElement>, PackingError>` - A Result containing a vector of unpacked
///   FieldElement values if successful, or an error if unsuccessful.
pub fn unpack(
    mut packed: Vec<FieldElement>,
    layout: Vec<FieldElement>,
) -> Result<Vec<FieldElement>, PackingError> {
    packed.reverse();
    let mut unpacked = vec![];

    let mut unpacking: U256 =
        packed.pop().ok_or(PackingError::UnpackingEntityError)?.as_ref().into();
    let mut offset = 0;

    // Iterate over the layout.
    for size in layout {
        let size: u8 = size.try_into().map_err(ParseError::ValueOutOfRange)?;
        let size: usize = size.into();
        let remaining_bits = 251 - offset;

        // If there are less remaining bits than the size, move to the next felt for unpacking.
        if remaining_bits < size {
            unpacking = packed.pop().ok_or(PackingError::UnpackingEntityError)?.as_ref().into();
            offset = 0;
        }

        let mut mask = U256::from(0_u8);
        for _ in 0..size {
            mask = (mask << 1) | U256::from(1_u8);
        }

        let result = mask & (unpacking >> offset);
        let result_fe =
            FieldElement::from_hex_be(&result.to_string()).map_err(ParseError::FromStr)?;
        unpacked.push(result_fe);

        // Update unpacking to be the shifted value after extracting the result.
        offset += size;
    }

    Ok(unpacked)
}

/// Parse a raw schema of a model into a Cairo type, [Ty]
pub fn parse_ty(data: &[FieldElement]) -> Result<Ty, ParseError> {
    let member_type: u8 = data[0].try_into()?;
    match member_type {
        0 => parse_simple(&data[1..]),
        1 => parse_struct(&data[1..]),
        2 => parse_enum(&data[1..]),
        3 => parse_tuple(&data[1..]),
        _ => Err(ParseError::InvalidSchema),
    }
}

fn parse_simple(data: &[FieldElement]) -> Result<Ty, ParseError> {
    let ty = parse_cairo_short_string(&data[0])?;
    Ok(Ty::Primitive(Primitive::from_str(&ty).expect("must be valid schema")))
}

fn parse_struct(data: &[FieldElement]) -> Result<Ty, ParseError> {
    let name = parse_cairo_short_string(&data[0])?;

    let attrs_len: u32 = data[1].try_into()?;
    let attrs_slice_start = 2;
    let attrs_slice_end = attrs_slice_start + attrs_len as usize;
    let _attrs = &data[attrs_slice_start..attrs_slice_end];

    let children_len: u32 = data[attrs_slice_end].try_into()?;
    let children_len = children_len as usize;

    let mut children = vec![];
    let mut offset = attrs_slice_end + 1;

    for i in 0..children_len {
        let start = i + offset;
        let len: u32 = data[start].try_into()?;
        let slice_start = start + 1;
        let slice_end = slice_start + len as usize;
        children.push(parse_member(&data[slice_start..slice_end])?);
        offset += len as usize;
    }

    Ok(Ty::Struct(schema::Struct { name, children }))
}

fn parse_member(data: &[FieldElement]) -> Result<schema::Member, ParseError> {
    let name = parse_cairo_short_string(&data[0])?;

    let attributes_len: u32 = data[1].try_into()?;
    let slice_start = 2;
    let slice_end = slice_start + attributes_len as usize;
    let attributes = &data[slice_start..slice_end];

    let key = attributes.contains(&cairo_short_string_to_felt("key")?);
    let ty = parse_ty(&data[slice_end..])?;

    Ok(schema::Member { name, ty, key })
}

fn parse_enum(data: &[FieldElement]) -> Result<Ty, ParseError> {
    let name = parse_cairo_short_string(&data[0])?;

    let attrs_len: u32 = data[1].try_into()?;
    let attrs_slice_start = 2;
    let attrs_slice_end = attrs_slice_start + attrs_len as usize;
    let _attrs = &data[attrs_slice_start..attrs_slice_end];

    let values_len: u32 = data[attrs_slice_end].try_into()?;
    let values_len = values_len as usize;

    let mut values = vec![];
    let mut offset = attrs_slice_end + 1;

    for i in 0..values_len {
        let start = i + offset;
        let name = parse_cairo_short_string(&data[start])?;
        let slice_start = start + 2;
        let len: u32 = data[start + 3].try_into()?;
        let len = len + 1; // Account for Ty enum index

        let slice_end = slice_start + len as usize;
        values.push((name, parse_ty(&data[slice_start..slice_end])?));
        offset += len as usize + 2;
    }

    Ok(Ty::Enum(schema::Enum { name, option: None, options: values }))
}

fn parse_tuple(data: &[FieldElement]) -> Result<Ty, ParseError> {
    if data.is_empty() {
        return Ok(Ty::Tuple(vec![]));
    }

    let children_len: u32 = data[0].try_into()?;
    let children_len = children_len as usize;

    let mut children = vec![];
    let mut offset = 1;

    for i in 0..children_len {
        let start = i + offset;
        let len: u32 = data[start].try_into()?;
        let slice_start = start + 1;
        let slice_end = slice_start + len as usize;
        children.push(parse_ty(&data[slice_start..slice_end])?);
        offset += len as usize;
    }

    Ok(Ty::Tuple(children))
}
