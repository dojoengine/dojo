use dojo_types::system::Dependency;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, FieldElement, FunctionCall, InvokeTransactionResult};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, parse_cairo_short_string,
    CairoShortStringToFeltError, ParseCairoShortStringError,
};
use starknet::providers::{Provider, ProviderError};

use crate::contract::world::{
    ContractReaderError, WorldContract, WorldContractError, WorldContractReader,
};

#[cfg(test)]
#[path = "system_test.rs"]
mod test;

#[derive(Debug, thiserror::Error)]
pub enum SystemError<S, P> {
    #[error(transparent)]
    WorldError(WorldContractError<S, P>),
    #[error(transparent)]
    ProviderError(ProviderError<P>),
    #[error(transparent)]
    ParseCairoShortStringError(ParseCairoShortStringError),
    #[error(transparent)]
    CairoShortStringToFeltError(CairoShortStringToFeltError),
    #[error(transparent)]
    ContractReaderError(ContractReaderError<P>),
    #[error(transparent)]
    ReaderError(SystemReaderError<P>),
}

pub struct System<'a, A: ConnectedAccount + Sync> {
    world: &'a WorldContract<'a, A>,
    reader: SystemReader<'a, A::Provider>,
    name: String,
}

impl<'a, A: ConnectedAccount + Sync> System<'a, A> {
    pub async fn new(
        world: &'a WorldContract<'a, A>,
        name: String,
        block_id: BlockId,
    ) -> Result<System<'a, A>, SystemError<A::SignError, <A::Provider as Provider>::Error>> {
        Ok(Self {
            name: name.clone(),
            world,
            reader: SystemReader::new(&world.reader, name, block_id)
                .await
                .map_err(SystemError::ReaderError)?,
        })
    }

    pub fn class_hash(&self) -> FieldElement {
        self.reader.class_hash
    }

    pub async fn call(
        &self,
        calldata: Vec<FieldElement>,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, SystemError<A::SignError, <A::Provider as Provider>::Error>>
    {
        self.reader.call(calldata, block_id).await.map_err(SystemError::ReaderError)
    }

    pub async fn dependencies(
        &self,
        block_id: BlockId,
    ) -> Result<Vec<Dependency>, SystemError<A::SignError, <A::Provider as Provider>::Error>> {
        self.reader.dependencies(block_id).await.map_err(SystemError::ReaderError)
    }

    pub async fn execute(
        &self,
        calldata: Vec<FieldElement>,
    ) -> Result<InvokeTransactionResult, SystemError<A::SignError, <A::Provider as Provider>::Error>>
    {
        let res =
            self.world.execute(&self.name, calldata).await.map_err(SystemError::WorldError)?;

        Ok(res)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SystemReaderError<P> {
    #[error(transparent)]
    ProviderError(ProviderError<P>),
    #[error(transparent)]
    ParseCairoShortStringError(ParseCairoShortStringError),
    #[error(transparent)]
    CairoShortStringToFeltError(CairoShortStringToFeltError),
    #[error(transparent)]
    ContractReaderError(ContractReaderError<P>),
    #[error("Invalid dependency length")]
    InvalidDependencyLength,
}

pub struct SystemReader<'a, P: Provider + Sync> {
    world: &'a WorldContractReader<'a, P>,
    name: String,
    class_hash: FieldElement,
}

impl<'a, P: Provider + Sync> SystemReader<'a, P> {
    pub async fn new(
        world: &'a WorldContractReader<'a, P>,
        name: String,
        block_id: BlockId,
    ) -> Result<SystemReader<'a, P>, SystemReaderError<P::Error>> {
        let res = world
            .provider
            .call(
                FunctionCall {
                    contract_address: world.address,
                    calldata: vec![cairo_short_string_to_felt(&name)
                        .map_err(SystemReaderError::CairoShortStringToFeltError)?],
                    entry_point_selector: get_selector_from_name("system").unwrap(),
                },
                block_id,
            )
            .await
            .map_err(SystemReaderError::ProviderError)?;

        Ok(Self { name, world, class_hash: res[0] })
    }

    pub fn class_hash(&self) -> FieldElement {
        self.class_hash
    }

    pub async fn call(
        &self,
        mut calldata: Vec<FieldElement>,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, SystemReaderError<P::Error>> {
        calldata.insert(0, (calldata.len() as u64).into());

        let res = self
            .world
            .call(&self.name, calldata, block_id)
            .await
            .map_err(SystemReaderError::ContractReaderError)?;

        Ok(res)
    }

    pub async fn dependencies(
        &self,
        block_id: BlockId,
    ) -> Result<Vec<Dependency>, SystemReaderError<P::Error>> {
        let entrypoint = get_selector_from_name("dependencies").unwrap();

        let res = self
            .world
            .call(
                "library_call",
                vec![FieldElement::THREE, self.class_hash, entrypoint, FieldElement::ZERO],
                block_id,
            )
            .await
            .map_err(SystemReaderError::ContractReaderError)?;

        let mut dependencies = vec![];
        for chunk in res[3..].chunks(2) {
            if chunk.len() != 2 {
                return Err(SystemReaderError::InvalidDependencyLength);
            }

            let is_write: bool = chunk[1] == FieldElement::ONE;

            dependencies.push(Dependency {
                name: parse_cairo_short_string(&chunk[0])
                    .map_err(SystemReaderError::ParseCairoShortStringError)?,
                read: !is_write,
                write: is_write,
            });
        }

        Ok(dependencies)
    }
}
