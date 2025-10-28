//! The deployer is in charge of deploying contracts to starknet.

use starknet::accounts::ConnectedAccount;
use starknet::core::types::{
    BlockId, BlockTag, Call, Felt, InvokeTransactionResult, StarknetError,
};
use starknet::core::utils::get_contract_address;
use starknet::macros::{felt, selector};
use starknet::providers::{Provider, ProviderError};
use tracing::trace;

use crate::{TransactionError, TransactionExt, TransactionResult, TransactionWaiter, TxnConfig};

const UDC_DEPLOY_SELECTOR: Felt = selector!("deployContract");
const UDC_ADDRESS: Felt =
    felt!("0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf");

#[derive(Debug)]
pub struct Deployer<A>
where
    A: ConnectedAccount + Send + Sync,
{
    /// The account to use to deploy the contracts.
    pub account: A,
    /// The transaction configuration.
    pub txn_config: TxnConfig,
}

impl<A> Deployer<A>
where
    A: ConnectedAccount + Send + Sync,
{
    /// Creates a new deployer.
    pub fn new(account: A, txn_config: TxnConfig) -> Self {
        Self { account, txn_config }
    }

    /// Get a Call for deploying a contract via the UDC.
    pub async fn deploy_via_udc_getcall(
        &self,
        class_hash: Felt,
        salt: Felt,
        constructor_calldata: &[Felt],
        deployer_address: Felt,
    ) -> Result<Option<(Felt, Call)>, TransactionError<A::SignError>> {
        let udc_calldata = [
            vec![class_hash, salt, deployer_address, Felt::from(constructor_calldata.len())],
            constructor_calldata.to_vec(),
        ]
        .concat();

        let contract_address =
            get_contract_address(salt, class_hash, constructor_calldata, deployer_address);

        if is_deployed(contract_address, &self.account.provider()).await? {
            return Ok(None);
        }

        Ok(Some((
            contract_address,
            Call { calldata: udc_calldata, selector: UDC_DEPLOY_SELECTOR, to: UDC_ADDRESS },
        )))
    }

    /// Deploys a contract via the UDC.
    pub async fn deploy_via_udc(
        &self,
        class_hash: Felt,
        salt: Felt,
        constructor_calldata: &[Felt],
        deployer_address: Felt,
    ) -> Result<(Felt, TransactionResult), TransactionError<A::SignError>> {
        let (contract_address, call) = match self
            .deploy_via_udc_getcall(class_hash, salt, constructor_calldata, deployer_address)
            .await?
        {
            Some(res) => res,
            None => return Ok((Felt::ZERO, TransactionResult::Noop)),
        };

        let InvokeTransactionResult { transaction_hash } =
            self.account.execute_v3(vec![call]).send_with_cfg(&self.txn_config).await?;

        trace!(
            transaction_hash = format!("{:#066x}", transaction_hash),
            contract_address = format!("{:#066x}", contract_address),
            "Deployed contract via UDC."
        );

        if self.txn_config.wait {
            let receipt = TransactionWaiter::new(transaction_hash, &self.account.provider())
                .with_tx_status(self.txn_config.finality_status)
                .await?;

            if self.txn_config.receipt {
                return Ok((
                    contract_address,
                    TransactionResult::HashReceipt(transaction_hash, Box::new(receipt)),
                ));
            }
        }

        Ok((contract_address, TransactionResult::Hash(transaction_hash)))
    }
}

/// Checks if a contract is deployed at the given address.
pub async fn is_deployed<P>(contract_address: Felt, provider: &P) -> Result<bool, ProviderError>
where
    P: Provider,
{
    match provider.get_class_hash_at(BlockId::Tag(BlockTag::PreConfirmed), contract_address).await {
        Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => Ok(false),
        Ok(_) => {
            trace!(
                contract_address = format!("{:#066x}", contract_address),
                "Contract already deployed."
            );
            Ok(true)
        }
        Err(e) => Err(e),
    }
}
