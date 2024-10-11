use jsonrpsee::core::{async_trait, RpcResult};
use katana_executor::ExecutorFactory;
use katana_pool::TransactionPool;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash};
use katana_rpc_api::starknet::StarknetWriteApiServer;
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::transaction::{
    BroadcastedDeclareTx, BroadcastedDeployAccountTx, BroadcastedInvokeTx, DeclareTxResult,
    DeployAccountTxResult, InvokeTxResult,
};

use super::StarknetApi;

impl<EF: ExecutorFactory> StarknetApi<EF> {
    async fn add_invoke_transaction_impl(
        &self,
        tx: BroadcastedInvokeTx,
    ) -> Result<InvokeTxResult, StarknetApiError> {
        self.on_io_blocking_task(move |this| {
            if tx.is_query() {
                return Err(StarknetApiError::UnsupportedTransactionVersion);
            }

            let tx = tx.into_tx_with_chain_id(this.inner.backend.chain_id);
            let tx = ExecutableTxWithHash::new(ExecutableTx::Invoke(tx));
            let hash =
                this.inner.pool.add_transaction(tx).inspect_err(|e| println!("Error: {:?}", e))?;

            Ok(hash.into())
        })
        .await
    }

    async fn add_declare_transaction_impl(
        &self,
        tx: BroadcastedDeclareTx,
    ) -> Result<DeclareTxResult, StarknetApiError> {
        self.on_io_blocking_task(move |this| {
            if tx.is_query() {
                return Err(StarknetApiError::UnsupportedTransactionVersion);
            }

            let tx = tx
                .try_into_tx_with_chain_id(this.inner.backend.chain_id)
                .map_err(|_| StarknetApiError::InvalidContractClass)?;

            let class_hash = tx.class_hash();
            let tx = ExecutableTxWithHash::new(ExecutableTx::Declare(tx));
            let hash = this.inner.pool.add_transaction(tx)?;

            Ok((hash, class_hash).into())
        })
        .await
    }

    async fn add_deploy_account_transaction_impl(
        &self,
        tx: BroadcastedDeployAccountTx,
    ) -> Result<DeployAccountTxResult, StarknetApiError> {
        self.on_io_blocking_task(move |this| {
            if tx.is_query() {
                return Err(StarknetApiError::UnsupportedTransactionVersion);
            }

            let tx = tx.into_tx_with_chain_id(this.inner.backend.chain_id);
            let contract_address = tx.contract_address();

            let tx = ExecutableTxWithHash::new(ExecutableTx::DeployAccount(tx));
            let hash = this.inner.pool.add_transaction(tx)?;

            Ok((hash, contract_address).into())
        })
        .await
    }
}

#[async_trait]
impl<EF: ExecutorFactory> StarknetWriteApiServer for StarknetApi<EF> {
    async fn add_invoke_transaction(
        &self,
        invoke_transaction: BroadcastedInvokeTx,
    ) -> RpcResult<InvokeTxResult> {
        Ok(self.add_invoke_transaction_impl(invoke_transaction).await?)
    }

    async fn add_declare_transaction(
        &self,
        declare_transaction: BroadcastedDeclareTx,
    ) -> RpcResult<DeclareTxResult> {
        Ok(self.add_declare_transaction_impl(declare_transaction).await?)
    }

    async fn add_deploy_account_transaction(
        &self,
        deploy_account_transaction: BroadcastedDeployAccountTx,
    ) -> RpcResult<DeployAccountTxResult> {
        Ok(self.add_deploy_account_transaction_impl(deploy_account_transaction).await?)
    }
}
