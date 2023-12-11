use std::vec;

use async_trait::async_trait;
use dojo_types::packing::{parse_ty, unpack, PackingError, ParseError};
use dojo_types::primitive::PrimitiveError;
use dojo_types::schema::Ty;
use starknet::core::types::{FieldElement, FunctionCall, StarknetError};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, CairoShortStringToFeltError,
    ParseCairoShortStringError,
};
use starknet::macros::short_string;
use starknet::providers::{Provider, ProviderError};
use starknet_crypto::poseidon_hash_many;

use crate::contracts::world::{ContractReaderError, WorldContractReader};

const WORLD_MODEL_SELECTOR_STR: &str = "model";
const SCHEMA_SELECTOR_STR: &str = "schema";
const LAYOUT_SELECTOR_STR: &str = "layout";
const PACKED_SIZE_SELECTOR_STR: &str = "packed_size";
const UNPACKED_SIZE_SELECTOR_STR: &str = "unpacked_size";

#[cfg(test)]
#[path = "model_test.rs"]
mod model_test;

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("Model not found.")]
    ModelNotFound,
    #[error(transparent)]
    ProviderError(#[from] ProviderError),
    #[error(transparent)]
    ParseCairoShortStringError(#[from] ParseCairoShortStringError),
    #[error(transparent)]
    CairoShortStringToFeltError(#[from] CairoShortStringToFeltError),
    #[error(transparent)]
    ContractReaderError(#[from] ContractReaderError),
    #[error(transparent)]
    CairoTypeError(#[from] PrimitiveError),
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Packing(#[from] PackingError),
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ModelReader<E> {
    fn class_hash(&self) -> FieldElement;
    async fn schema(&self) -> Result<Ty, E>;
    async fn packed_size(&self) -> Result<FieldElement, E>;
    async fn unpacked_size(&self) -> Result<FieldElement, E>;
    async fn layout(&self) -> Result<Vec<FieldElement>, E>;
}

pub struct ModelRPCReader<'a, P: Sync + Send> {
    /// The name of the model
    name: FieldElement,
    /// The class hash of the model
    class_hash: FieldElement,
    /// Contract reader of the World that the model is registered to.
    world_reader: &'a WorldContractReader<P>,
}

impl<'a, P> ModelRPCReader<'a, P>
where
    P: Provider + Sync + Send,
{
    pub async fn new(
        name: &str,
        world: &'a WorldContractReader<P>,
    ) -> Result<ModelRPCReader<'a, P>, ModelError> {
        let name = cairo_short_string_to_felt(name)?;

        let class_hash = world
            .provider()
            .call(
                FunctionCall {
                    calldata: vec![name],
                    contract_address: world.address(),
                    entry_point_selector: get_selector_from_name(WORLD_MODEL_SELECTOR_STR).unwrap(),
                },
                world.block_id(),
            )
            .await
            .map(|res| res[0])
            .map_err(|err| match err {
                ProviderError::StarknetError(StarknetError::ContractNotFound) => {
                    ModelError::ModelNotFound
                }
                err => err.into(),
            })?;

        Ok(Self { world_reader: world, class_hash, name })
    }

    pub async fn entity_storage(
        &self,
        keys: &[FieldElement],
    ) -> Result<Vec<FieldElement>, ModelError> {
        let packed_size: u8 =
            self.packed_size().await?.try_into().map_err(ParseError::ValueOutOfRange)?;

        let key = poseidon_hash_many(keys);
        let key = poseidon_hash_many(&[short_string!("dojo_storage"), self.name, key]);

        let mut packed = Vec::with_capacity(packed_size as usize);
        for slot in 0..packed_size {
            let value = self
                .world_reader
                .provider()
                .get_storage_at(
                    self.world_reader.address(),
                    key + slot.into(),
                    self.world_reader.block_id(),
                )
                .await?;

            packed.push(value);
        }

        Ok(packed)
    }

    pub async fn entity(&self, keys: &[FieldElement]) -> Result<Ty, ModelError> {
        let mut schema = self.schema().await?;

        let layout = self.layout().await?;
        let raw_values = self.entity_storage(keys).await?;

        let unpacked = unpack(raw_values, layout)?;
        let mut keys_and_unpacked = [keys, &unpacked].concat();

        schema.deserialize(&mut keys_and_unpacked)?;

        Ok(schema)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<'a, P> ModelReader<ModelError> for ModelRPCReader<'a, P>
where
    P: Provider + Sync + Send,
{
    fn class_hash(&self) -> FieldElement {
        self.class_hash
    }

    async fn schema(&self) -> Result<Ty, ModelError> {
        let entrypoint = get_selector_from_name(SCHEMA_SELECTOR_STR).unwrap();

        let res = self
            .world_reader
            .executor_call(self.class_hash, vec![entrypoint, FieldElement::ZERO])
            .await?;

        Ok(parse_ty(&res[1..])?)
    }

    async fn packed_size(&self) -> Result<FieldElement, ModelError> {
        let entrypoint = get_selector_from_name(PACKED_SIZE_SELECTOR_STR).unwrap();

        let res = self
            .world_reader
            .executor_call(self.class_hash, vec![entrypoint, FieldElement::ZERO])
            .await?;

        Ok(res[1])
    }

    async fn unpacked_size(&self) -> Result<FieldElement, ModelError> {
        let entrypoint = get_selector_from_name(UNPACKED_SIZE_SELECTOR_STR).unwrap();

        let res = self
            .world_reader
            .executor_call(self.class_hash, vec![entrypoint, FieldElement::ZERO])
            .await?;

        Ok(res[1])
    }

    async fn layout(&self) -> Result<Vec<FieldElement>, ModelError> {
        let entrypoint = get_selector_from_name(LAYOUT_SELECTOR_STR).unwrap();

        let res = self
            .world_reader
            .executor_call(self.class_hash, vec![entrypoint, FieldElement::ZERO])
            .await?;

        Ok(res[2..].into())
    }
}
