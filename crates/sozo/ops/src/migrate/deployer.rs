//! The deployer is in charge of deploying contracts to starknet.

use dojo_utils::{TransactionExt, TransactionWaiter, TxnConfig};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{
    BlockId, BlockTag, Call, Felt, InvokeTransactionResult, StarknetError,
};
use starknet::core::utils::get_contract_address;
use starknet::macros::{felt, selector};
use starknet::providers::{Provider, ProviderError};

use super::MigrationError;

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

    /// Deploys a contract via the UDC.
    pub async fn deploy_via_udc(
        &self,
        class_hash: Felt,
        salt: Felt,
        constructor_calldata: &[Felt],
        deployer_address: Felt,
    ) -> Result<Felt, MigrationError<A::SignError>> {
        let udc_calldata = [
            vec![
                class_hash,                             // class hash
                salt,                                   // salt
                deployer_address,                       // unique
                Felt::from(constructor_calldata.len()), // constructor calldata len
            ],
            constructor_calldata.to_vec(),
        ]
        .concat();

        let contract_address =
            get_contract_address(salt, class_hash, &constructor_calldata, deployer_address);

        if is_deployed(contract_address, &self.account.provider())
            .await
            .map_err(MigrationError::Provider)?
        {
            return Ok(Felt::ZERO);
        }

        let txn = self.account.execute_v1(vec![Call {
            calldata: udc_calldata,
            selector: UDC_DEPLOY_SELECTOR,
            to: UDC_ADDRESS,
        }]);

        let InvokeTransactionResult { transaction_hash } =
            txn.send_with_cfg(&self.txn_config).await.map_err(MigrationError::from)?;

        tracing::trace!(
            transaction_hash = format!("{:#066x}", transaction_hash),
            contract_address = format!("{:#066x}", contract_address),
            "Deployed contract via UDC."
        );

        if self.txn_config.wait {
            TransactionWaiter::new(transaction_hash, &self.account.provider()).await?;
        }

        Ok(contract_address)
    }
}

/// Checks if a contract is deployed at the given address.
pub async fn is_deployed<P>(contract_address: Felt, provider: &P) -> Result<bool, ProviderError>
where
    P: Provider,
{
    match provider.get_class_hash_at(BlockId::Tag(BlockTag::Pending), contract_address).await {
        Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => Ok(false),
        Ok(_) => {
            tracing::trace!(
                contract_address = format!("{:#066x}", contract_address),
                "Contract already deployed."
            );
            return Ok(true);
        }
        Err(e) => Err(e),
    }
}
