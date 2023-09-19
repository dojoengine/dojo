use std::vec;

use crypto_bigint::U256;
use dojo_types::component::{Enum, Member, Struct, Ty};
use starknet::core::types::{BlockId, FieldElement, FunctionCall};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, parse_cairo_short_string,
    CairoShortStringToFeltError, ParseCairoShortStringError,
};
use starknet::macros::short_string;
use starknet::providers::{Provider, ProviderError};
use starknet_crypto::poseidon_hash_many;

use crate::contract::world::{ContractReaderError, WorldContractReader};

#[cfg(test)]
#[path = "component_test.rs"]
mod test;

#[derive(Debug, thiserror::Error)]
pub enum ComponentError<P> {
    #[error(transparent)]
    ProviderError(ProviderError<P>),
    #[error("Invalid schema")]
    InvalidSchema,
    #[error(transparent)]
    ParseCairoShortStringError(ParseCairoShortStringError),
    #[error(transparent)]
    CairoShortStringToFeltError(CairoShortStringToFeltError),
    #[error("Converting felt")]
    ConvertingFelt,
    #[error("Unpacking entity")]
    UnpackingEntity,
    #[error(transparent)]
    ContractReaderError(ContractReaderError<P>),
}

pub struct ComponentReader<'a, P: Provider + Sync> {
    world: &'a WorldContractReader<'a, P>,
    class_hash: FieldElement,
    name: FieldElement,
}

impl<'a, P: Provider + Sync> ComponentReader<'a, P> {
    pub async fn new(
        world: &'a WorldContractReader<'a, P>,
        name: String,
        block_id: BlockId,
    ) -> Result<ComponentReader<'a, P>, ComponentError<P::Error>> {
        let name = cairo_short_string_to_felt(&name)
            .map_err(ComponentError::CairoShortStringToFeltError)?;
        let res = world
            .provider
            .call(
                FunctionCall {
                    contract_address: world.address,
                    calldata: vec![name],
                    entry_point_selector: get_selector_from_name("component").unwrap(),
                },
                block_id,
            )
            .await
            .map_err(ComponentError::ProviderError)?;

        Ok(Self { world, class_hash: res[0], name })
    }

    pub fn class_hash(&self) -> FieldElement {
        self.class_hash
    }

    pub async fn schema(&self, block_id: BlockId) -> Result<Ty, ComponentError<P::Error>> {
        let entrypoint = get_selector_from_name("schema").unwrap();

        let res = self
            .world
            .call(
                "library_call",
                vec![FieldElement::THREE, self.class_hash, entrypoint, FieldElement::ZERO],
                block_id,
            )
            .await
            .map_err(ComponentError::ContractReaderError)?;

        parse_ty::<P>(&res[2..])
    }

    pub async fn size(&self, block_id: BlockId) -> Result<FieldElement, ComponentError<P::Error>> {
        let entrypoint = get_selector_from_name("size").unwrap();

        let res = self
            .world
            .call(
                "library_call",
                vec![FieldElement::THREE, self.class_hash, entrypoint, FieldElement::ZERO],
                block_id,
            )
            .await
            .map_err(ComponentError::ContractReaderError)?;

        Ok(res[2])
    }

    pub async fn layout(
        &self,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, ComponentError<P::Error>> {
        let entrypoint = get_selector_from_name("layout").unwrap();

        let res = self
            .world
            .call(
                "library_call",
                vec![FieldElement::THREE, self.class_hash, entrypoint, FieldElement::ZERO],
                block_id,
            )
            .await
            .map_err(ComponentError::ContractReaderError)?;

        Ok(res[3..].into())
    }

    pub async fn entity(
        &self,
        keys: Vec<FieldElement>,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, ComponentError<P::Error>> {
        let size: u8 = self.size(block_id).await?.try_into().unwrap();
        let layout = self.layout(block_id).await?;

        let key = poseidon_hash_many(&keys);
        let key = poseidon_hash_many(&[short_string!("dojo_storage"), self.name, key]);

        let mut packed = vec![];
        for slot in 0..size {
            let value = self
                .world
                .provider
                .get_storage_at(self.world.address, key + slot.into(), block_id)
                .await
                .map_err(ComponentError::ProviderError)?;

            packed.push(value);
        }

        let unpacked = unpack::<P>(packed, layout)?;

        Ok(unpacked)
    }
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
/// * `Result<Vec<FieldElement>, ComponentError<P::Error>>` - A Result containing a vector of
///   unpacked FieldElement values if successful, or an error if unsuccessful.
pub fn unpack<P: Provider>(
    mut packed: Vec<FieldElement>,
    layout: Vec<FieldElement>,
) -> Result<Vec<FieldElement>, ComponentError<P::Error>> {
    packed.reverse();
    let mut unpacked = vec![];

    let mut unpacking: U256 = packed.pop().ok_or(ComponentError::UnpackingEntity)?.as_ref().into();
    let mut offset = 0;

    // Iterate over the layout.
    for size in layout {
        let size: u8 = size.try_into().map_err(|_| ComponentError::ConvertingFelt)?;
        let size: usize = size.into();
        let remaining_bits = 251 - offset;

        // If there are less remaining bits than the size, move to the next felt for unpacking.
        if remaining_bits < size {
            unpacking = packed.pop().ok_or(ComponentError::UnpackingEntity)?.as_ref().into();
            offset = 0;
        }

        let mut mask = U256::from(0_u8);
        for _ in 0..size {
            mask = (mask << 1) | U256::from(1_u8);
        }

        let result = mask & (unpacking >> offset);
        let result_fe = FieldElement::from_hex_be(&result.to_string())
            .map_err(|_| ComponentError::ConvertingFelt)?;
        unpacked.push(result_fe);

        // Update unpacking to be the shifted value after extracting the result.
        offset += size;
    }

    Ok(unpacked)
}

fn parse_ty<P: Provider>(data: &[FieldElement]) -> Result<Ty, ComponentError<P::Error>> {
    let member_type: u8 = data[0].try_into().unwrap();
    match member_type {
        0 => parse_simple::<P>(&data[1..]),
        1 => parse_struct::<P>(&data[1..]),
        2 => parse_enum::<P>(&data[1..]),
        _ => Err(ComponentError::InvalidSchema),
    }
}

fn parse_simple<P: Provider>(data: &[FieldElement]) -> Result<Ty, ComponentError<P::Error>> {
    let ty =
        parse_cairo_short_string(&data[0]).map_err(ComponentError::ParseCairoShortStringError)?;
    Ok(Ty::Simple(ty))
}

fn parse_struct<P: Provider>(data: &[FieldElement]) -> Result<Ty, ComponentError<P::Error>> {
    let name =
        parse_cairo_short_string(&data[0]).map_err(ComponentError::ParseCairoShortStringError)?;

    let attrs_len: u32 = data[1].try_into().unwrap();
    let attrs_slice_start = 2;
    let attrs_slice_end = attrs_slice_start + attrs_len as usize;
    let _attrs = &data[attrs_slice_start..attrs_slice_end];

    let children_len: u32 = data[attrs_slice_end].try_into().unwrap();
    let children_len = children_len as usize;

    let mut children = vec![];
    let mut offset = attrs_slice_end + 1;

    for i in 0..children_len {
        let start = i + offset;
        let len: u32 = data[start].try_into().unwrap();
        let slice_start = start + 1;
        let slice_end = slice_start + len as usize;
        children.push(parse_member::<P>(&data[slice_start..slice_end])?);
        offset += len as usize;
    }

    Ok(Ty::Struct(Struct { name, children }))
}

fn parse_member<P: Provider>(data: &[FieldElement]) -> Result<Member, ComponentError<P::Error>> {
    let name =
        parse_cairo_short_string(&data[0]).map_err(ComponentError::ParseCairoShortStringError)?;

    let attributes_len: u32 = data[1].try_into().unwrap();
    let slice_start = 2;
    let slice_end = slice_start + attributes_len as usize;
    let attributes = &data[slice_start..slice_end];

    let key = attributes.contains(&cairo_short_string_to_felt("key").unwrap());

    let ty = parse_ty::<P>(&data[slice_end..])?;

    Ok(Member { name, ty, key })
}

fn parse_enum<P: Provider>(data: &[FieldElement]) -> Result<Ty, ComponentError<P::Error>> {
    let name =
        parse_cairo_short_string(&data[0]).map_err(ComponentError::ParseCairoShortStringError)?;

    let attrs_len: u32 = data[1].try_into().unwrap();
    let attrs_slice_start = 2;
    let attrs_slice_end = attrs_slice_start + attrs_len as usize;
    let _attrs = &data[attrs_slice_start..attrs_slice_end];

    let values_len: u32 = data[attrs_slice_end].try_into().unwrap();
    let values_len = values_len as usize;

    let mut values = vec![];
    let mut offset = attrs_slice_end + 1;

    for i in 0..values_len {
        let start = i + offset;
        let len: u32 = data[start].try_into().unwrap();
        let slice_start = start + 1;
        let slice_end = slice_start + len as usize;
        values.push(parse_ty::<P>(&data[slice_start..slice_end])?);
        offset += len as usize;
    }

    Ok(Ty::Enum(Enum { name, values }))
}
