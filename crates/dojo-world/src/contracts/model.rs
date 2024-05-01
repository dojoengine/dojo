pub use abigen::model::ModelContractReader;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde, ContractAddress, Error as CainomeError};
use dojo_types::packing::{parse_ty, PackingError, ParseError};
use dojo_types::primitive::PrimitiveError;
use dojo_types::schema::Ty;
use starknet::core::types::FieldElement;
use starknet::core::utils::{
    get_selector_from_name, CairoShortStringToFeltError, NonAsciiNameError,
    ParseCairoShortStringError,
};
use starknet::providers::{Provider, ProviderError};

use crate::contracts::WorldContractReader;

#[cfg(test)]
#[path = "model_test.rs"]
mod model_test;

pub mod abigen {
    pub mod model {
        pub use crate::contracts::abi::model::*;
    }
    pub mod world {
        pub use crate::contracts::abi::world::*;
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
    NonAsciiNameError(#[from] NonAsciiNameError),
    #[error(transparent)]
    CairoTypeError(#[from] PrimitiveError),
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Packing(#[from] PackingError),
    #[error(transparent)]
    Cainome(#[from] CainomeError),
}

// TODO: to update to match with new model interface
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ModelReader<E> {
    // TODO: kept for compatibility but should be removed
    // because it returns the model name hash and not the model name itself.
    fn name(&self) -> String;
    fn selector(&self) -> FieldElement;
    fn class_hash(&self) -> FieldElement;
    fn contract_address(&self) -> FieldElement;
    async fn schema(&self) -> Result<Ty, E>;
    async fn packed_size(&self) -> Result<Option<u32>, E>;
    async fn unpacked_size(&self) -> Result<Option<u32>, E>;
    async fn layout(&self) -> Result<abigen::model::Layout, E>;
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
        let selector = get_selector_from_name(name)?;

        let (class_hash, contract_address) =
            world.model(&selector).block_id(world.block_id).call().await?;

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
            name: selector,
            model_reader,
        })
    }

    pub async fn entity_storage(
        &self,
        keys: &[FieldElement],
    ) -> Result<Vec<FieldElement>, ModelError> {
        // As the dojo::database::introspect::Layout type has been pasted
        // in both `model` and `world` ABI by abigen, the compiler sees both types
        // as different even if they are strictly identical.
        // Here is a trick reading the model layout as raw FieldElement
        // and deserialize it to a world::Layout.
        let raw_layout = self.model_reader.layout().raw_call().await?;
        let layout = abigen::world::Layout::cairo_deserialize(raw_layout.as_slice(), 0)?;

        Ok(self.world_reader.entity(&self.selector(), &keys.to_vec(), &layout).call().await?)
    }

    pub async fn entity(&self, keys: &[FieldElement]) -> Result<Ty, ModelError> {
        let mut schema = self.schema().await?;
        let values = self.entity_storage(keys).await?;

        let mut keys_and_unpacked = [keys, &values].concat();

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

    fn selector(&self) -> FieldElement {
        self.name
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

    async fn packed_size(&self) -> Result<Option<u32>, ModelError> {
        Ok(self.model_reader.packed_size().call().await?)
    }

    async fn unpacked_size(&self) -> Result<Option<u32>, ModelError> {
        Ok(self.model_reader.unpacked_size().call().await?)
    }

    async fn layout(&self) -> Result<abigen::model::Layout, ModelError> {
        Ok(self.model_reader.layout().call().await?)
    }
}
