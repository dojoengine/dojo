use anyhow::Result;
use starknet::accounts::{AccountError, Call, ConnectedAccount};
use starknet::core::types::{
    BlockId, BlockTag, FieldElement, FunctionCall, InvokeTransactionResult,
};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::{Provider, ProviderError};

use crate::component::ComponentClass;

#[cfg(test)]
#[path = "world_test.rs"]
mod test;

#[derive(Debug)]
pub struct WorldContractWriter<'a, A: ConnectedAccount + Sync> {
    pub address: FieldElement,
    pub account: &'a A,
}

impl<'a, A: ConnectedAccount + Sync> WorldContractWriter<'a, A> {
    pub fn new(address: FieldElement, account: &'a A) -> Self {
        Self { address, account }
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

    pub async fn register_systems(
        &self,
        systems: &[FieldElement],
    ) -> Result<InvokeTransactionResult, AccountError<A::SignError, <A::Provider as Provider>::Error>>
    {
        let calls = systems
            .iter()
            .map(|s| Call {
                to: self.address,
                // function selector: "register_system"
                selector: FieldElement::from_mont([
                    6581716859078500959,
                    16871126355047595269,
                    14219012428168968926,
                    473332093618875024,
                ]),
                calldata: vec![*s],
            })
            .collect::<Vec<_>>();

        self.account.execute(calls).send().await
    }

    pub async fn execute(
        &self,
        name: FieldElement,
        mut calldata: Vec<FieldElement>,
    ) -> Result<InvokeTransactionResult, AccountError<A::SignError, <A::Provider as Provider>::Error>>
    {
        calldata.insert(0, name);
        self.account
            .execute(vec![Call {
                calldata,
                to: self.address,
                selector: get_selector_from_name("execute").unwrap(),
            }])
            .send()
            .await
    }
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

    pub async fn executor(
        &self,
        block_id: BlockId,
    ) -> Result<FieldElement, ProviderError<<P as starknet::providers::Provider>::Error>> {
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
            .await?;

        Ok(res[0])
    }

    pub async fn call(
        &self,
        name: FieldElement,
        mut calldata: Vec<FieldElement>,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, ProviderError<<P as starknet::providers::Provider>::Error>> {
        calldata.insert(0, name);
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
    }

    pub async fn component(
        &'a self,
        name: FieldElement,
        block_id: BlockId,
    ) -> Result<ComponentClass<'a, P>, ProviderError<P::Error>> {
        ComponentClass::new(self, name, block_id).await
    }

    pub async fn system(
        &self,
        name: FieldElement,
    ) -> Result<Vec<FieldElement>, ProviderError<P::Error>> {
        self.provider
            .call(
                FunctionCall {
                    contract_address: self.address,
                    calldata: vec![name],
                    entry_point_selector: get_selector_from_name("system").unwrap(),
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await
    }
}
