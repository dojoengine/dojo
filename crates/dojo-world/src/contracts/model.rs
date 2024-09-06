use std::str::FromStr as _;

use async_trait::async_trait;
use cainome::cairo_serde::Error as CainomeError;
use dojo_types::packing::{PackingError, ParseError};
use dojo_types::primitive::{Primitive, PrimitiveError};
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use starknet::core::types::Felt;
use starknet::core::utils::{
    cairo_short_string_to_felt, parse_cairo_short_string, CairoShortStringToFeltError,
    NonAsciiNameError, ParseCairoShortStringError,
};
use starknet::providers::{Provider, ProviderError};

use super::abi::world::ModelIndex;
use super::naming;
use crate::contracts::WorldContractReader;

#[cfg(test)]
#[path = "model_test.rs"]
mod model_test;

pub mod abigen {
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
    #[error("{0}")]
    TagError(String),
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ModelReader<E> {
    fn namespace(&self) -> String;
    fn name(&self) -> String;
    fn selector(&self) -> Felt;
    fn layout(&self) -> abigen::world::Layout;
    fn ty(&self) -> abigen::world::Ty;
    fn packed_size(&self) -> u32;
    fn unpacked_size(&self) -> u32;
    fn schema(&self) -> Result<Ty, ModelError>;
}

#[derive(Debug)]
pub struct ModelRPCReader<'a, P: Provider + Sync + Send> {
    definition: abigen::world::ModelDefinition,

    /// Contract reader of the World that the model is registered to.
    world_reader: &'a WorldContractReader<P>,
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
        let model_selector = naming::compute_selector_from_names(namespace, name);

        let model_definition =
            match world.resource(&model_selector).block_id(world.block_id).call().await? {
                abigen::world::Resource::Model(definition) => definition,
                _ => return Err(ModelError::ModelNotFound),
            };

        Ok(Self { definition: model_definition, world_reader: world })
    }

    pub async fn entity_storage(&self, keys: &[Felt]) -> Result<Vec<Felt>, ModelError> {
        Ok(self
            .world_reader
            .entity(&self.selector(), &ModelIndex::Keys(keys.to_vec()), &self.layout())
            .call()
            .await?)
    }

    pub async fn entity(&self, keys: &[Felt]) -> Result<Ty, ModelError> {
        let mut schema = self.schema()?;
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
    fn namespace(&self) -> String {
        self.definition.namespace.to_string().unwrap()
    }

    fn name(&self) -> String {
        self.definition.name.to_string().unwrap()
    }

    fn selector(&self) -> Felt {
        self.definition.selector
    }

    fn layout(&self) -> abigen::world::Layout {
        self.definition.layout.clone()
    }

    fn ty(&self) -> abigen::world::Ty {
        self.definition.ty.clone()
    }

    fn schema(&self) -> Result<Ty, ModelError> {
        parse_schema(&self.definition.ty).map_err(ModelError::Parse)
    }

    fn packed_size(&self) -> u32 {
        // TODO: would be better to directly return an Option.
        self.definition.packed_size.unwrap_or(0)
    }

    fn unpacked_size(&self) -> u32 {
        // TODO: would be better to directly return an Option.
        self.definition.unpacked_size.unwrap_or(0)
    }
}

fn parse_schema(ty: &abigen::world::Ty) -> Result<Ty, ParseError> {
    match ty {
        abigen::world::Ty::Primitive(primitive) => {
            let ty = parse_cairo_short_string(primitive)?;
            let ty = ty.split("::").last().unwrap();
            let primitive = match Primitive::from_str(ty) {
                Ok(primitive) => primitive,
                Err(_) => return Err(ParseError::invalid_schema()),
            };

            Ok(Ty::Primitive(primitive))
        }
        abigen::world::Ty::Struct(schema) => {
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
        abigen::world::Ty::Enum(enum_) => {
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
        abigen::world::Ty::Tuple(values) => {
            let values = values.iter().map(parse_schema).collect::<Result<Vec<_>, ParseError>>()?;

            Ok(Ty::Tuple(values))
        }
        abigen::world::Ty::Array(values) => {
            let values = values.iter().map(parse_schema).collect::<Result<Vec<_>, ParseError>>()?;

            Ok(Ty::Array(values))
        }
        abigen::world::Ty::ByteArray => Ok(Ty::ByteArray("".to_string())),
    }
}
