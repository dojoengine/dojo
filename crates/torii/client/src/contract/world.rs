use std::result::Result;

use starknet::accounts::{AccountError, Call, ConnectedAccount};
use starknet::core::types::{BlockId, FieldElement, FunctionCall, InvokeTransactionResult};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, CairoShortStringToFeltError,
};
use starknet::providers::{Provider, ProviderError};

use crate::contract::model::{ModelError, ModelReader};

#[cfg(test)]
#[path = "world_test.rs"]
pub(crate) mod test;

#[derive(Debug, thiserror::Error)]
pub enum WorldContractError<S, P> {
    #[error(transparent)]
    ProviderError(ProviderError<P>),
    #[error(transparent)]
    AccountError(AccountError<S, P>),
    #[error(transparent)]
    CairoShortStringToFeltError(CairoShortStringToFeltError),
    #[error(transparent)]
    ContractReaderError(ContractReaderError<P>),
}

#[derive(Debug)]
pub struct WorldContract<'a, A: ConnectedAccount + Sync> {
    pub address: FieldElement,
    pub account: &'a A,
    pub reader: WorldContractReader<'a, A::Provider>,
}

impl<'a, A: ConnectedAccount + Sync> WorldContract<'a, A> {
    pub fn new(address: FieldElement, account: &'a A) -> Self {
        Self { address, account, reader: WorldContractReader::new(address, account.provider()) }
    }

    pub async fn set_executor(
        &self,
        executor: FieldElement,
    ) -> Result<InvokeTransactionResult, AccountError<A::SignError, <A::Provider as Provider>::Error>>
    {
        self.account
            .execute(vec![Call {
                calldata: vec![executor],
                to: self.address,
                selector: get_selector_from_name("set_executor").unwrap(),
            }])
            .send()
            .await
    }

    pub async fn grant_writer(
        &self,
        model: &str,
        system: &str,
    ) -> Result<
        InvokeTransactionResult,
        WorldContractError<A::SignError, <A::Provider as Provider>::Error>,
    > {
        let component = cairo_short_string_to_felt(model)
            .map_err(WorldContractError::CairoShortStringToFeltError)?;
        let system = cairo_short_string_to_felt(system)
            .map_err(WorldContractError::CairoShortStringToFeltError)?;

        self.account
            .execute(vec![Call {
                calldata: vec![component, system],
                to: self.address,
                selector: get_selector_from_name("grant_writer").unwrap(),
            }])
            .send()
            .await
            .map_err(WorldContractError::AccountError)
    }

    pub async fn register_components(
        &self,
        components: &[FieldElement],
    ) -> Result<InvokeTransactionResult, AccountError<A::SignError, <A::Provider as Provider>::Error>>
    {
        let calls = components
            .iter()
            .map(|c| Call {
                to: self.address,
                // function selector: "register_component"
                selector: FieldElement::from_mont([
                    11981012454229264524,
                    8784065169116922201,
                    15056747385353365869,
                    456849768949735353,
                ]),
                calldata: vec![*c],
            })
            .collect::<Vec<_>>();

        self.account.execute(calls).send().await
    }

    pub async fn executor(
        &self,
        block_id: BlockId,
    ) -> Result<FieldElement, ContractReaderError<<A::Provider as Provider>::Error>> {
        self.reader.executor(block_id).await
    }

    pub async fn call(
        &self,
        system: &str,
        calldata: Vec<FieldElement>,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, ContractReaderError<<A::Provider as Provider>::Error>> {
        self.reader.call(system, calldata, block_id).await
    }

    pub async fn component(
        &'a self,
        name: &str,
        block_id: BlockId,
    ) -> Result<ModelReader<'a, A::Provider>, ModelError<<A::Provider as Provider>::Error>> {
        self.reader.model(name, block_id).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContractReaderError<P> {
    #[error(transparent)]
    ProviderError(ProviderError<P>),
    #[error(transparent)]
    CairoShortStringToFeltError(CairoShortStringToFeltError),
}

#[derive(Debug)]
pub struct WorldContractReader<'a, P: Provider + Sync> {
    pub address: FieldElement,
    pub provider: &'a P,
}

impl<'a, P: Provider + Sync> WorldContractReader<'a, P> {
    pub fn new(address: FieldElement, provider: &'a P) -> Self {
        Self { address, provider }
    }

    pub async fn is_authorized(
        &self,
        system: &str,
        component: &str,
        execution_role: &str,
        block_id: BlockId,
    ) -> Result<bool, ContractReaderError<P::Error>> {
        let res = self
            .provider
            .call(
                FunctionCall {
                    contract_address: self.address,
                    calldata: vec![
                        cairo_short_string_to_felt(system)
                            .map_err(ContractReaderError::CairoShortStringToFeltError)?,
                        cairo_short_string_to_felt(component)
                            .map_err(ContractReaderError::CairoShortStringToFeltError)?,
                        cairo_short_string_to_felt(execution_role)
                            .map_err(ContractReaderError::CairoShortStringToFeltError)?,
                    ],
                    entry_point_selector: get_selector_from_name("is_authorized").unwrap(),
                },
                block_id,
            )
            .await
            .map_err(ContractReaderError::ProviderError)?;

        Ok(res[0] == FieldElement::ONE)
    }

    pub async fn is_account_admin(
        &self,
        block_id: BlockId,
    ) -> Result<bool, ContractReaderError<P::Error>> {
        let res = self
            .provider
            .call(
                FunctionCall {
                    contract_address: self.address,
                    calldata: vec![],
                    entry_point_selector: get_selector_from_name("is_account_admin").unwrap(),
                },
                block_id,
            )
            .await
            .map_err(ContractReaderError::ProviderError)?;

        Ok(res[0] == FieldElement::ONE)
    }

    pub async fn executor(
        &self,
        block_id: BlockId,
    ) -> Result<FieldElement, ContractReaderError<P::Error>> {
        let res = self
            .provider
            .call(
                FunctionCall {
                    contract_address: self.address,
                    calldata: vec![],
                    entry_point_selector: get_selector_from_name("executor").unwrap(),
                },
                block_id,
            )
            .await
            .map_err(ContractReaderError::ProviderError)?;

        Ok(res[0])
    }

    pub async fn executor_call(
        &self,
        class_hash: FieldElement,
        mut calldata: Vec<FieldElement>,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, ContractReaderError<P::Error>> {
        calldata.insert(0, class_hash);

        self.provider
            .call(
                FunctionCall {
                    contract_address: self.executor(block_id).await.unwrap(),
                    calldata,
                    entry_point_selector: get_selector_from_name("call").unwrap(),
                },
                block_id,
            )
            .await
            .map_err(ContractReaderError::ProviderError)
    }

    pub async fn call(
        &self,
        system: &str,
        mut calldata: Vec<FieldElement>,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, ContractReaderError<P::Error>> {
        calldata.insert(
            0,
            cairo_short_string_to_felt(system)
                .map_err(ContractReaderError::CairoShortStringToFeltError)?,
        );
        self.provider
            .call(
                FunctionCall {
                    contract_address: self.address,
                    calldata,
                    entry_point_selector: get_selector_from_name("execute").unwrap(),
                },
                block_id,
            )
            .await
            .map_err(ContractReaderError::ProviderError)
    }

    pub async fn model(
        &'a self,
        name: &str,
        block_id: BlockId,
    ) -> Result<ModelReader<'a, P>, ModelError<P::Error>> {
        ModelReader::new(self, name.to_string(), block_id).await
    }
}
