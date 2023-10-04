use std::vec;

use dojo_types::packing::{parse_ty, unpack, PackingError, ParseError};
use dojo_types::primitive::PrimitiveError;
use dojo_types::schema::Ty;
use starknet::core::types::{BlockId, FieldElement, FunctionCall};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, CairoShortStringToFeltError,
    ParseCairoShortStringError,
};
use starknet::macros::short_string;
use starknet::providers::{Provider, ProviderError};
use starknet_crypto::poseidon_hash_many;

use crate::contract::world::{ContractReaderError, WorldContractReader};

const WORLD_MODEL_SELECTOR_STR: &str = "model";
const SCHEMA_SELECTOR_STR: &str = "schema";
const LAYOUT_SELECTOR_STR: &str = "layout";
const PACKED_SIZE_SELECTOR_STR: &str = "packed_size";
const UNPACKED_SIZE_SELECTOR_STR: &str = "unpacked_size";

#[cfg(test)]
#[path = "model_test.rs"]
mod model_test;

#[derive(Debug, thiserror::Error)]
pub enum ModelError<P> {
    #[error(transparent)]
    ProviderError(ProviderError<P>),
    #[error(transparent)]
    ParseCairoShortStringError(ParseCairoShortStringError),
    #[error(transparent)]
    CairoShortStringToFeltError(CairoShortStringToFeltError),
    #[error(transparent)]
    ContractReaderError(ContractReaderError<P>),
    #[error(transparent)]
    CairoTypeError(PrimitiveError),
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Packing(#[from] PackingError),
}

pub struct ModelReader<'a, P: Provider + Sync> {
    world: &'a WorldContractReader<'a, P>,
    class_hash: FieldElement,
    name: FieldElement,
}

impl<'a, P: Provider + Sync> ModelReader<'a, P> {
    pub async fn new(
        world: &'a WorldContractReader<'a, P>,
        name: String,
        block_id: BlockId,
    ) -> Result<ModelReader<'a, P>, ModelError<P::Error>> {
        let name =
            cairo_short_string_to_felt(&name).map_err(ModelError::CairoShortStringToFeltError)?;
        let res = world
            .provider
            .call(
                FunctionCall {
                    contract_address: world.address,
                    calldata: vec![name],
                    entry_point_selector: get_selector_from_name(WORLD_MODEL_SELECTOR_STR).unwrap(),
                },
                block_id,
            )
            .await
            .map_err(ModelError::ProviderError)?;

        Ok(Self { world, class_hash: res[0], name })
    }

    pub fn class_hash(&self) -> FieldElement {
        self.class_hash
    }

    pub async fn schema(&self, block_id: BlockId) -> Result<Ty, ModelError<P::Error>> {
        let entrypoint = get_selector_from_name(SCHEMA_SELECTOR_STR).unwrap();

        let res = self
            .world
            .executor_call(self.class_hash, vec![entrypoint, FieldElement::ZERO], block_id)
            .await
            .map_err(ModelError::ContractReaderError)?;

        Ok(parse_ty(&res[1..])?)
    }

    pub async fn packed_size(
        &self,
        block_id: BlockId,
    ) -> Result<FieldElement, ModelError<P::Error>> {
        let entrypoint = get_selector_from_name(PACKED_SIZE_SELECTOR_STR).unwrap();

        let res = self
            .world
            .executor_call(self.class_hash, vec![entrypoint, FieldElement::ZERO], block_id)
            .await
            .map_err(ModelError::ContractReaderError)?;

        Ok(res[1])
    }

    pub async fn unpacked_size(
        &self,
        block_id: BlockId,
    ) -> Result<FieldElement, ModelError<P::Error>> {
        let entrypoint = get_selector_from_name(UNPACKED_SIZE_SELECTOR_STR).unwrap();

        let res = self
            .world
            .executor_call(self.class_hash, vec![entrypoint, FieldElement::ZERO], block_id)
            .await
            .map_err(ModelError::ContractReaderError)?;

        Ok(res[1])
    }

    pub async fn layout(
        &self,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, ModelError<P::Error>> {
        let entrypoint = get_selector_from_name(LAYOUT_SELECTOR_STR).unwrap();

        let res = self
            .world
            .executor_call(self.class_hash, vec![entrypoint, FieldElement::ZERO], block_id)
            .await
            .map_err(ModelError::ContractReaderError)?;

        Ok(res[2..].into())
    }

    pub async fn entity_storage(
        &self,
        keys: Vec<FieldElement>,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, ModelError<P::Error>> {
        let packed_size: u8 = self.packed_size(block_id).await?.try_into().unwrap();

        let key = poseidon_hash_many(&keys);
        let key = poseidon_hash_many(&[short_string!("dojo_storage"), self.name, key]);

        let mut packed = vec![];
        for slot in 0..packed_size {
            let value = self
                .world
                .provider
                .get_storage_at(self.world.address, key + slot.into(), block_id)
                .await
                .map_err(ModelError::ProviderError)?;

            packed.push(value);
        }

        Ok(packed)
    }

    pub async fn entity(
        &self,
        keys: Vec<FieldElement>,
        block_id: BlockId,
    ) -> Result<Ty, ModelError<P::Error>> {
        let mut schema = self.schema(block_id).await?;

        let layout = self.layout(block_id).await?;
        let raw_values = self.entity_storage(keys.clone(), block_id).await?;

        let unpacked = unpack(raw_values, layout.clone())?;
        let mut keys_and_unpacked = [keys, unpacked].concat();

        let _ = schema.deserialize(&mut keys_and_unpacked).map_err(ModelError::CairoTypeError::<P>);

        Ok(schema)
    }
}
