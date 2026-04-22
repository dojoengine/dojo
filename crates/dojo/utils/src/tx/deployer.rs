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

    /// Get the deterministic UDC-derived contract address along with the
    /// Call required to deploy it (or `None` for the Call if the contract
    /// is already deployed at that address).
    ///
    /// The address is always returned, even on the already-deployed path,
    /// so callers don't have to re-derive it themselves.
    pub async fn deploy_via_udc_getcall(
        &self,
        class_hash: Felt,
        salt: Felt,
        constructor_calldata: &[Felt],
        deployer_address: Felt,
    ) -> Result<(Felt, Option<Call>), TransactionError<A::SignError>> {
        let udc_calldata = [
            vec![class_hash, salt, deployer_address, Felt::from(constructor_calldata.len())],
            constructor_calldata.to_vec(),
        ]
        .concat();

        let contract_address =
            get_contract_address(salt, class_hash, constructor_calldata, deployer_address);

        if is_deployed(contract_address, &self.account.provider()).await? {
            return Ok((contract_address, None));
        }

        Ok((
            contract_address,
            Some(Call { calldata: udc_calldata, selector: UDC_DEPLOY_SELECTOR, to: UDC_ADDRESS }),
        ))
    }

    /// Deploys a contract via the UDC.
    pub async fn deploy_via_udc(
        &self,
        class_hash: Felt,
        salt: Felt,
        constructor_calldata: &[Felt],
        deployer_address: Felt,
    ) -> Result<(Felt, TransactionResult), TransactionError<A::SignError>> {
        let (contract_address, call) = self
            .deploy_via_udc_getcall(class_hash, salt, constructor_calldata, deployer_address)
            .await?;
        let Some(call) = call else {
            return Ok((contract_address, TransactionResult::Noop));
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
    match provider.get_class_hash_at(BlockId::Tag(BlockTag::Latest), contract_address).await {
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

#[cfg(test)]
mod tests {
    use katana_runner::RunnerCtx;
    use starknet::core::utils::get_contract_address;
    use starknet::macros::felt;

    use super::*;
    use crate::TxnConfig;

    // The default account class that katana dev predeclares on every chain.
    // Used as the class_hash for our deploy tests so we don't need to declare
    // a contract first.
    const KATANA_DEV_ACCOUNT_CLASS_HASH: Felt =
        felt!("0x07dc7899aa655b0aae51eadff6d801a58e97dd99cf4666ee59e704249e51adf2");

    /// Regression: `deploy_via_udc_getcall` used to return `Option<(Felt, Call)>`
    /// where `None` meant "already deployed" and the address was dropped on
    /// the floor. `deploy_via_udc` then mapped that to `(Felt::ZERO, Noop)`.
    /// After the fix both paths surface the real contract address, so
    /// deploy is idempotent across re-runs with the same salt.
    #[tokio::test(flavor = "multi_thread")]
    #[katana_runner::test(accounts = 2)]
    async fn deploy_via_udc_idempotent_returns_real_address(sequencer: &RunnerCtx) {
        let account = sequencer.account(0);
        let deployer = Deployer::new(account, TxnConfig { wait: true, ..Default::default() });

        let class_hash = KATANA_DEV_ACCOUNT_CLASS_HASH;
        let salt = felt!("0xabc");
        // Account class has a single-arg constructor (public_key). Any non-zero
        // felt works for this test; we never interact with the deployed account.
        let calldata = vec![felt!("0xdeadbeef")];
        let deployer_address = Felt::ZERO;

        let expected_address = get_contract_address(salt, class_hash, &calldata, deployer_address);

        // First call: not yet deployed. Returns (addr, Some(call)).
        let (addr, call) = deployer
            .deploy_via_udc_getcall(class_hash, salt, &calldata, deployer_address)
            .await
            .unwrap();
        assert_eq!(addr, expected_address);
        assert!(call.is_some(), "expected deploy Call on the not-yet-deployed path");

        // Actually deploy it.
        let (deployed_addr, _tx) =
            deployer.deploy_via_udc(class_hash, salt, &calldata, deployer_address).await.unwrap();
        assert_eq!(deployed_addr, expected_address);

        // Second getcall with identical params: contract is already deployed
        // at the same address. Returns (same addr, None) — this is the path
        // that used to lose the address before the fix.
        let (addr, call) = deployer
            .deploy_via_udc_getcall(class_hash, salt, &calldata, deployer_address)
            .await
            .unwrap();
        assert_eq!(addr, expected_address, "address must be surfaced even when already deployed");
        assert!(call.is_none(), "no deploy Call needed on the already-deployed path");

        // Second deploy_via_udc call: returns (real_address, Noop). Before
        // the fix this returned (Felt::ZERO, Noop).
        let (addr, tx) =
            deployer.deploy_via_udc(class_hash, salt, &calldata, deployer_address).await.unwrap();
        assert_eq!(addr, expected_address);
        assert!(
            matches!(tx, TransactionResult::Noop),
            "already-deployed path must return Noop, got {tx:?}"
        );
    }
}
