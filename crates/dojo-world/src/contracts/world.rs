use std::result::Result;

use http::uri::{InvalidUri, Uri};
use starknet::accounts::{AccountError, Call, ConnectedAccount};
use starknet::core::types::{
    BlockId, BlockTag, FieldElement, FunctionCall, InvokeTransactionResult,
};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, CairoShortStringToFeltError,
};
use starknet::macros::selector;
use starknet::providers::{Provider, ProviderError};

use super::model::{ModelError, ModelReader};

#[cfg(test)]
#[path = "world_test.rs"]
pub(crate) mod test;

#[derive(Debug, thiserror::Error)]
pub enum WorldContractError<S, P> {
    #[error(transparent)]
    ProviderError(#[from] ProviderError<P>),
    #[error(transparent)]
    AccountError(#[from] AccountError<S, P>),
    #[error(transparent)]
    CairoShortStringToFeltError(#[from] CairoShortStringToFeltError),
    #[error(transparent)]
    ContractReaderError(#[from] ContractReaderError<P>),
    #[error("Invalid metadata uri")]
    InvalidMetadataUri(#[from] InvalidUri),
}

pub struct WorldContract<'a, A>
where
    A: ConnectedAccount,
{
    account: &'a A,
    reader: WorldContractReader<&'a <A as ConnectedAccount>::Provider>,
}

impl<'a, A> WorldContract<'a, A>
where
    A: ConnectedAccount,
{
    pub fn new(address: FieldElement, account: &'a A) -> Self {
        Self { account, reader: WorldContractReader::new(address, account.provider()) }
    }

    pub fn account(&self) -> &A {
        self.account
    }
}

impl<'a, A> WorldContract<'a, A>
where
    A: ConnectedAccount + Sync,
{
    pub async fn set_executor(
        &self,
        executor: FieldElement,
    ) -> Result<InvokeTransactionResult, AccountError<A::SignError, <A::Provider as Provider>::Error>>
    {
        self.account
            .execute(vec![Call {
                to: self.reader.address,
                calldata: vec![executor],
                selector: selector!("set_executor"),
            }])
            .send()
            .await
    }

    pub async fn set_metadata_uri(
        &self,
        resource: FieldElement,
        metadata_uri: String,
    ) -> Result<
        InvokeTransactionResult,
        WorldContractError<A::SignError, <A::Provider as Provider>::Error>,
    > {
        let parsed: Uri =
            metadata_uri.try_into().map_err(WorldContractError::InvalidMetadataUri)?;

        let mut encoded = parsed
            .to_string()
            .chars()
            .collect::<Vec<_>>()
            .chunks(31)
            .map(|chunk| {
                let s: String = chunk.iter().collect();
                cairo_short_string_to_felt(&s).unwrap()
            })
            .collect::<Vec<_>>();

        encoded.insert(0, encoded.len().into());
        encoded.insert(0, resource);

        self.account
            .execute(vec![Call {
                calldata: encoded,
                to: self.reader.address,
                selector: get_selector_from_name("set_metadata_uri").unwrap(),
            }])
            .send()
            .await
            .map_err(WorldContractError::AccountError)
    }

    pub async fn grant_writer(
        &self,
        model: &str,
        contract: FieldElement,
    ) -> Result<
        InvokeTransactionResult,
        WorldContractError<A::SignError, <A::Provider as Provider>::Error>,
    > {
        let model = cairo_short_string_to_felt(model)
            .map_err(WorldContractError::CairoShortStringToFeltError)?;

        self.account
            .execute(vec![Call {
                calldata: vec![model, contract],
                to: self.reader.address,
                selector: get_selector_from_name("grant_writer").unwrap(),
            }])
            .send()
            .await
            .map_err(WorldContractError::AccountError)
    }

    pub async fn register_models(
        &self,
        models: &[FieldElement],
    ) -> Result<InvokeTransactionResult, AccountError<A::SignError, <A::Provider as Provider>::Error>>
    {
        let calls = models
            .iter()
            .map(|c| Call {
                to: self.reader.address,
                selector: selector!("register_model"),
                calldata: vec![*c],
            })
            .collect::<Vec<_>>();

        self.account.execute(calls).send().await
    }

    pub async fn deploy_contract(
        &self,
        salt: &FieldElement,
        class_hash: &FieldElement,
    ) -> Result<InvokeTransactionResult, AccountError<A::SignError, <A::Provider as Provider>::Error>>
    {
        self.account
            .execute(vec![Call {
                to: self.reader.address,
                selector: selector!("deploy_contract"),
                calldata: vec![*salt, *class_hash],
            }])
            .send()
            .await
    }

    pub async fn executor(
        &self,
    ) -> Result<FieldElement, ContractReaderError<<A::Provider as Provider>::Error>> {
        self.reader.executor().await
    }

    pub async fn base(
        &self,
    ) -> Result<FieldElement, ContractReaderError<<A::Provider as Provider>::Error>> {
        self.reader.base().await
    }

    pub async fn model(
        &'a self,
        name: &str,
    ) -> Result<ModelReader<'_, &'a A::Provider>, ModelError<<A::Provider as Provider>::Error>>
    {
        self.reader.model(name).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContractReaderError<P> {
    #[error(transparent)]
    ProviderError(#[from] ProviderError<P>),
    #[error(transparent)]
    CairoShortStringToFeltError(#[from] CairoShortStringToFeltError),
}

pub struct WorldContractReader<P> {
    provider: P,
    block_id: BlockId,
    address: FieldElement,
}

impl<P> WorldContractReader<P>
where
    P: Provider,
{
    pub fn new(address: FieldElement, provider: P) -> Self {
        Self { address, provider, block_id: BlockId::Tag(BlockTag::Latest) }
    }

    pub fn with_block(self, block: BlockId) -> Self {
        Self { block_id: block, ..self }
    }

    pub fn address(&self) -> FieldElement {
        self.address
    }

    pub fn provider(&self) -> &P {
        &self.provider
    }

    pub fn block_id(&self) -> BlockId {
        self.block_id
    }
}

impl<P> WorldContractReader<P>
where
    P: Provider,
{
    pub async fn is_authorized(
        &self,
        system: &str,
        model: &str,
        execution_role: &str,
    ) -> Result<bool, ContractReaderError<P::Error>> {
        let res = self
            .provider
            .call(
                FunctionCall {
                    calldata: vec![
                        cairo_short_string_to_felt(system)?,
                        cairo_short_string_to_felt(model)?,
                        cairo_short_string_to_felt(execution_role)?,
                    ],
                    contract_address: self.address,
                    entry_point_selector: selector!("is_authorized"),
                },
                self.block_id,
            )
            .await?;

        Ok(res[0] == FieldElement::ONE)
    }

    pub async fn is_account_admin(&self) -> Result<bool, ContractReaderError<P::Error>> {
        let res = self
            .provider
            .call(
                FunctionCall {
                    calldata: vec![],
                    contract_address: self.address,
                    entry_point_selector: selector!("is_account_admin"),
                },
                self.block_id,
            )
            .await?;

        Ok(res[0] == FieldElement::ONE)
    }

    pub async fn executor(&self) -> Result<FieldElement, ContractReaderError<P::Error>> {
        let res = self
            .provider
            .call(
                FunctionCall {
                    calldata: vec![],
                    contract_address: self.address,
                    entry_point_selector: selector!("executor"),
                },
                self.block_id,
            )
            .await?;

        Ok(res[0])
    }

    pub async fn metadata_uri(&self) -> Result<FieldElement, ContractReaderError<P::Error>> {
        let res = self
            .provider
            .call(
                FunctionCall {
                    calldata: vec![],
                    contract_address: self.address,
                    entry_point_selector: selector!("metadata_uri"),
                },
                self.block_id,
            )
            .await?;

        Ok(res[0])
    }

    pub async fn base(&self) -> Result<FieldElement, ContractReaderError<P::Error>> {
        let res = self
            .provider
            .call(
                FunctionCall {
                    calldata: vec![],
                    contract_address: self.address,
                    entry_point_selector: selector!("base"),
                },
                self.block_id,
            )
            .await?;

        Ok(res[0])
    }

    pub async fn executor_call(
        &self,
        class_hash: FieldElement,
        mut calldata: Vec<FieldElement>,
    ) -> Result<Vec<FieldElement>, ContractReaderError<P::Error>> {
        calldata.insert(0, class_hash);

        let res = self
            .provider
            .call(
                FunctionCall {
                    calldata,
                    contract_address: self.executor().await?,
                    entry_point_selector: selector!("call"),
                },
                self.block_id,
            )
            .await?;

        Ok(res)
    }
}

impl<'a, P> WorldContractReader<P>
where
    P: Provider,
{
    pub async fn model(&'a self, name: &str) -> Result<ModelReader<'a, P>, ModelError<P::Error>> {
        ModelReader::new(name, self).await
    }
}
