use jsonrpsee::core::{async_trait, RpcResult};
use katana_executor::ExecutorFactory;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash};
use katana_rpc_api::starknet::StarknetWriteApiServer;
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::transaction::{
    BroadcastedDeclareTx, BroadcastedDeployAccountTx, BroadcastedInvokeTx, DeclareTxResult,
    DeployAccountTxResult, InvokeTxResult,
};

use super::StarknetApi;

#[async_trait]
impl<EF: ExecutorFactory> StarknetWriteApiServer for StarknetApi<EF> {
    async fn add_invoke_transaction(
        &self,
        invoke_transaction: BroadcastedInvokeTx,
    ) -> RpcResult<InvokeTxResult> {
        self.on_io_blocking_task(move |this| {
            if invoke_transaction.is_query() {
                return Err(StarknetApiError::UnsupportedTransactionVersion.into());
            }

            let chain_id = this.inner.sequencer.chain_id();

            let tx = invoke_transaction.into_tx_with_chain_id(chain_id);
            let tx = ExecutableTxWithHash::new(ExecutableTx::Invoke(tx));
            let tx_hash = tx.hash;

            this.inner.sequencer.add_transaction_to_pool(tx);

            Ok(tx_hash.into())
        })
        .await
    }

    async fn add_declare_transaction(
        &self,
        declare_transaction: BroadcastedDeclareTx,
    ) -> RpcResult<DeclareTxResult> {
        self.on_io_blocking_task(move |this| {
            if declare_transaction.is_query() {
                return Err(StarknetApiError::UnsupportedTransactionVersion.into());
            }

            let chain_id = this.inner.sequencer.chain_id();

            let tx = declare_transaction
                .try_into_tx_with_chain_id(chain_id)
                .map_err(|_| StarknetApiError::InvalidContractClass)?;

            let class_hash = tx.class_hash();
            let tx = ExecutableTxWithHash::new(ExecutableTx::Declare(tx));
            let tx_hash = tx.hash;

            this.inner.sequencer.add_transaction_to_pool(tx);

            Ok((tx_hash, class_hash).into())
        })
        .await
    }

    async fn add_deploy_account_transaction(
        &self,
        deploy_account_transaction: BroadcastedDeployAccountTx,
    ) -> RpcResult<DeployAccountTxResult> {
        self.on_io_blocking_task(move |this| {
            if deploy_account_transaction.is_query() {
                return Err(StarknetApiError::UnsupportedTransactionVersion.into());
            }

            let chain_id = this.inner.sequencer.chain_id();

            let tx = deploy_account_transaction.into_tx_with_chain_id(chain_id);
            let contract_address = tx.contract_address();

            let tx = ExecutableTxWithHash::new(ExecutableTx::DeployAccount(tx));
            let tx_hash = tx.hash;

            this.inner.sequencer.add_transaction_to_pool(tx);

            Ok((tx_hash, contract_address).into())
        })
        .await
    }
}
