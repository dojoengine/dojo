pub use abigen::model::ModelContractReader;
use async_trait::async_trait;
use cainome::cairo_serde::{ContractAddress, Error as CainomeError};
use dojo_types::packing::{parse_ty, unpack, PackingError, ParseError};
use dojo_types::primitive::PrimitiveError;
use dojo_types::schema::Ty;
use starknet::core::types::FieldElement;
use starknet::core::utils::{
    cairo_short_string_to_felt, CairoShortStringToFeltError, ParseCairoShortStringError,
};
use starknet::macros::short_string;
use starknet::providers::{Provider, ProviderError};
use starknet_crypto::poseidon_hash_many;

use crate::contracts::WorldContractReader;

#[cfg(test)]
#[path = "model_test.rs"]
mod model_test;

pub mod abigen {
    pub mod model {
        pub use crate::contracts::abi::model::*;
    }
}

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
    CairoTypeError(#[from] PrimitiveError),
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Packing(#[from] PackingError),
    #[error(transparent)]
    Cainome(#[from] CainomeError),
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ModelReader<E> {
    fn name(&self) -> String;
    fn class_hash(&self) -> FieldElement;
    fn contract_address(&self) -> FieldElement;
    async fn schema(&self) -> Result<Ty, E>;
    async fn packed_size(&self) -> Result<FieldElement, E>;
    async fn unpacked_size(&self) -> Result<FieldElement, E>;
    async fn layout(&self) -> Result<Vec<FieldElement>, E>;
}

pub struct ModelRPCReader<'a, P: Provider + Sync + Send> {
    /// The name of the model
    name: FieldElement,
    /// The class hash of the model
    class_hash: FieldElement,
    /// The contract address of the model
    contract_address: FieldElement,
    /// Contract reader of the World that the model is registered to.
    world_reader: &'a WorldContractReader<P>,
    /// Contract reader of the model.
    model_reader: ModelContractReader<&'a P>,
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

        let (class_hash, contract_address) =
            world.model(&name).block_id(world.block_id).call().await?;

        // World Cairo contract won't raise an error in case of unknown/unregistered
        // model so raise an error here in case of zero address.
        if contract_address == ContractAddress(FieldElement::ZERO) {
            return Err(ModelError::ModelNotFound);
        }

        let model_reader = ModelContractReader::new(contract_address.into(), world.provider());

        Ok(Self {
            world_reader: world,
            class_hash: class_hash.into(),
            contract_address: contract_address.into(),
            name,
            model_reader,
        })
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
                    self.world_reader.address,
                    key + slot.into(),
                    self.world_reader.block_id,
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
    fn name(&self) -> String {
        self.name.to_string()
    }

    fn class_hash(&self) -> FieldElement {
        self.class_hash
    }

    fn contract_address(&self) -> FieldElement {
        self.contract_address
    }

    async fn schema(&self) -> Result<Ty, ModelError> {
        let res = self.model_reader.schema().raw_call().await?;
        Ok(parse_ty(&res)?)
    }

    async fn packed_size(&self) -> Result<FieldElement, ModelError> {
        Ok(self.model_reader.packed_size().raw_call().await?[0])
    }

    async fn unpacked_size(&self) -> Result<FieldElement, ModelError> {
        Ok(self.model_reader.unpacked_size().raw_call().await?[0])
    }

    async fn layout(&self) -> Result<Vec<FieldElement>, ModelError> {
        // Layout entrypoint expanded by the #[model] attribute returns a
        // `Span`. So cainome generated code will deserialize the result
        // of `executor.call()` which is a Vec<FieldElement>.
        // So inside the vec, we skip the first element, which is the length
        // of the span returned by `layout` entrypoint of the model code.
        Ok(self.model_reader.layout().raw_call().await?[1..].into())
    }
}
