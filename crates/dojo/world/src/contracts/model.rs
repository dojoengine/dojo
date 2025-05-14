use std::str::FromStr as _;

use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde as _, ContractAddress, Error as CainomeError};
use dojo_types::packing::{PackingError, ParseError};
use dojo_types::primitive::{Primitive, PrimitiveError};
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use starknet::core::types::{BlockId, Felt};
use starknet::core::utils::{
    cairo_short_string_to_felt, parse_cairo_short_string, CairoShortStringToFeltError,
    NonAsciiNameError, ParseCairoShortStringError,
};
use starknet::providers::{Provider, ProviderError};

pub use super::abigen::model::ModelContractReader;
use super::abigen::world::{Layout, ModelIndex};
use super::{abigen, naming};
use crate::contracts::WorldContractReader;

// #[cfg(test)]
// #[path = "model_test.rs"]
// mod model_test;

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
    #[error("{0}")]
    TagError(String),
}

// TODO: to update to match with new model interface
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ModelReader<E> {
    fn namespace(&self) -> &str;
    fn name(&self) -> &str;
    fn selector(&self) -> Felt;
    fn class_hash(&self) -> Felt;
    fn contract_address(&self) -> Felt;
    async fn schema(&self) -> Result<Ty, E>;
    async fn packed_size(&self) -> Result<u32, E>;
    async fn unpacked_size(&self) -> Result<u32, E>;
    async fn layout(&self) -> Result<abigen::model::Layout, E>;
}

#[derive(Debug)]
pub struct ModelRPCReader<'a, P: Provider + Sync + Send> {
    /// Namespace of the model
    namespace: String,
    /// Name of the model
    name: String,
    /// The selector of the model
    selector: Felt,
    /// The class hash of the model
    class_hash: Felt,
    /// The contract address of the model
    contract_address: Felt,
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
        namespace: &str,
        name: &str,
        address: Felt,
        class_hash: Felt,
        world: &'a WorldContractReader<P>,
    ) -> ModelRPCReader<'a, P> {
        let selector = naming::compute_selector_from_names(namespace, name);
        let contract_reader = ModelContractReader::new(address, world.provider());

        Self {
            namespace: namespace.into(),
            name: name.into(),
            selector,
            class_hash,
            contract_address: address,
            world_reader: world,
            model_reader: contract_reader,
        }
    }

    pub async fn new_from_world(
        namespace: &str,
        name: &str,
        world: &'a WorldContractReader<P>,
    ) -> Result<ModelRPCReader<'a, P>, ModelError> {
        let model_selector = naming::compute_selector_from_names(namespace, name);

        // Events are also considered like models from a off-chain perspective. They both have
        // introspection and convey type information.
        let (contract_address, class_hash) =
            match world.resource(&model_selector).block_id(world.block_id).call().await? {
                abigen::world::Resource::Model((address, hash)) => (address, hash),
                abigen::world::Resource::Event((address, hash)) => (address, hash),
                _ => return Err(ModelError::ModelNotFound),
            };

        // World Cairo contract won't raise an error in case of unknown/unregistered
        // model so raise an error here in case of zero address.
        if contract_address == ContractAddress(Felt::ZERO) {
            return Err(ModelError::ModelNotFound);
        }

        Ok(Self::new(namespace, name, contract_address.0, class_hash, world).await)
    }

    pub async fn entity_storage(&self, keys: &[Felt]) -> Result<Vec<Felt>, ModelError> {
        // As the dojo::model::Layout type has been pasted
        // in both `model` and `world` ABI by abigen, the compiler sees both types
        // as different even if they are strictly identical.
        // Here is a trick reading the model layout as raw FieldElement
        // and deserialize it to a world::Layout.
        let raw_layout = self.model_reader.layout().raw_call().await?;
        let layout = Layout::cairo_deserialize(raw_layout.as_slice(), 0)?;

        Ok(self
            .world_reader
            .entity(&self.selector(), &ModelIndex::Keys(keys.to_vec()), &layout)
            .call()
            .await?)
    }

    pub async fn entity(&self, keys: &[Felt]) -> Result<Ty, ModelError> {
        let mut schema = self.schema().await?;
        let values = self.entity_storage(keys).await?;

        let mut keys_and_unpacked = [keys, &values].concat();

        schema.deserialize(&mut keys_and_unpacked)?;

        Ok(schema)
    }

    pub async fn set_block(&mut self, block_id: BlockId) {
        self.model_reader.set_block(block_id);
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<'a, P> ModelReader<ModelError> for ModelRPCReader<'a, P>
where
    P: Provider + Sync + Send,
{
    fn namespace(&self) -> &str {
        &self.namespace
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn selector(&self) -> Felt {
        self.selector
    }

    fn class_hash(&self) -> Felt {
        self.class_hash
    }

    fn contract_address(&self) -> Felt {
        self.contract_address
    }

    async fn schema(&self) -> Result<Ty, ModelError> {
        let res = self.model_reader.schema().call().await?;
        parse_schema(&abigen::model::Ty::Struct(res)).map_err(ModelError::Parse)
    }

    // For non fixed layouts,   packed and unpacked sizes are None.
    // Therefore we return 0 in this case.
    async fn packed_size(&self) -> Result<u32, ModelError> {
        Ok(self.model_reader.packed_size().call().await?.unwrap_or(0))
    }

    async fn unpacked_size(&self) -> Result<u32, ModelError> {
        Ok(self.model_reader.unpacked_size().call().await?.unwrap_or(0))
    }

    async fn layout(&self) -> Result<abigen::model::Layout, ModelError> {
        Ok(self.model_reader.layout().call().await?)
    }

    async fn use_legacy_storage(&self) -> Result<bool, ModelError> {
        Ok(self.model_reader.use_legacy_storage().call().await?)
    }
}

fn parse_schema(ty: &abigen::model::Ty) -> Result<Ty, ParseError> {
    match ty {
        abigen::model::Ty::Primitive(primitive) => {
            let ty = parse_cairo_short_string(primitive)?;
            let ty = ty.split("::").last().unwrap();
            let primitive = match Primitive::from_str(ty) {
                Ok(primitive) => primitive,
                Err(_) => return Err(ParseError::invalid_schema()),
            };

            Ok(Ty::Primitive(primitive))
        }
        abigen::model::Ty::Struct(schema) => {
            let name = parse_cairo_short_string(&schema.name)?;

            let children = schema
                .children
                .iter()
                .map(|child| {
                    Ok(Member {
                        name: parse_cairo_short_string(&child.name)?,
                        ty: parse_schema(&child.ty)?,
                        key: child.attrs.contains(&cairo_short_string_to_felt("key").unwrap()),
                    })
                })
                .collect::<Result<Vec<_>, ParseError>>()?;

            Ok(Ty::Struct(Struct { name, children }))
        }
        abigen::model::Ty::Enum(enum_) => {
            let mut name = parse_cairo_short_string(&enum_.name)?;

            let options = enum_
                .children
                .iter()
                .map(|(variant_name, ty)| {
                    // strip "(T)" of the type of the enum variant for now
                    // breaks the db queries
                    // Some(T) => Some
                    let mut variant_name = parse_cairo_short_string(variant_name)?;

                    let ty = parse_schema(ty)?;
                    // generalize this for any generic name?
                    if variant_name.ends_with("(T)") {
                        variant_name = variant_name.trim_end_matches("(T)").to_string();
                        name = name.replace("<T>", format!("<{}>", ty.name()).as_str());
                    }

                    Ok(EnumOption { name: variant_name, ty })
                })
                .collect::<Result<Vec<_>, ParseError>>()?;

            Ok(Ty::Enum(Enum { name, option: None, options }))
        }
        abigen::model::Ty::Tuple(values) => {
            let values = values.iter().map(parse_schema).collect::<Result<Vec<_>, ParseError>>()?;

            Ok(Ty::Tuple(values))
        }
        abigen::model::Ty::Array(values) => {
            let values = values.iter().map(parse_schema).collect::<Result<Vec<_>, ParseError>>()?;

            Ok(Ty::Array(values))
        }
        abigen::model::Ty::ByteArray => Ok(Ty::ByteArray("".to_string())),
    }
}
