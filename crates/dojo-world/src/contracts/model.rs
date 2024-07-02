use std::str::FromStr as _;

pub use abigen::model::ModelContractReader;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde as _, ContractAddress, Error as CainomeError};
use dojo_types::packing::{PackingError, ParseError};
use dojo_types::primitive::{Primitive, PrimitiveError};
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use starknet::core::types::FieldElement;
use starknet::core::utils::{
    cairo_short_string_to_felt, parse_cairo_short_string, CairoShortStringToFeltError,
    NonAsciiNameError, ParseCairoShortStringError,
};
use starknet::providers::{Provider, ProviderError};

use super::abi::world::Layout;
use crate::contracts::WorldContractReader;
use crate::manifest::utils::compute_model_selector_from_names;

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
    fn selector(&self) -> FieldElement;
    fn class_hash(&self) -> FieldElement;
    fn contract_address(&self) -> FieldElement;
    async fn schema(&self) -> Result<Ty, E>;
    async fn packed_size(&self) -> Result<u32, E>;
    async fn unpacked_size(&self) -> Result<u32, E>;
    async fn layout(&self) -> Result<abigen::model::Layout, E>;
}

pub struct ModelRPCReader<'a, P: Provider + Sync + Send> {
    /// Namespace of the model
    namespace: String,
    /// Name of the model
    name: String,
    /// The selector of the model
    selector: FieldElement,
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
        namespace: &str,
        name: &str,
        world: &'a WorldContractReader<P>,
    ) -> Result<ModelRPCReader<'a, P>, ModelError> {
        let model_selector = compute_model_selector_from_names(namespace, name);

        let (class_hash, contract_address) =
            world.model(&model_selector).block_id(world.block_id).call().await?;

        // World Cairo contract won't raise an error in case of unknown/unregistered
        // model so raise an error here in case of zero address.
        if contract_address == ContractAddress(FieldElement::ZERO) {
            return Err(ModelError::ModelNotFound);
        }

        let model_reader = ModelContractReader::new(contract_address.into(), world.provider());

        Ok(Self {
            namespace: namespace.into(),
            name: name.into(),
            world_reader: world,
            class_hash: class_hash.into(),
            contract_address: contract_address.into(),
            selector: model_selector,
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
        let layout = Layout::cairo_deserialize(raw_layout.as_slice(), 0)?;

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
    fn namespace(&self) -> &str {
        &self.namespace
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn selector(&self) -> FieldElement {
        self.selector
    }

    fn class_hash(&self) -> FieldElement {
        self.class_hash
    }

    fn contract_address(&self) -> FieldElement {
        self.contract_address
    }

    async fn schema(&self) -> Result<Ty, ModelError> {
        let res = self.model_reader.schema().call().await?;
        parse_schema(&res).map_err(ModelError::Parse)
    }

    // For non fixed layouts, packed and unpacked sizes are None.
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
            let name = parse_cairo_short_string(&enum_.name)?;

            let options = enum_
                .children
                .iter()
                .map(|(name, ty)| {
                    // strip of the type (T) of the enum variant for now
                    // breaks the db queries
                    let name =
                        parse_cairo_short_string(name)?.split('(').next().unwrap().to_string();
                    let ty = parse_schema(ty)?;

                    Ok(EnumOption { name, ty })
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
