use std::sync::Arc;

use anyhow::anyhow;
use jsonrpsee::core::{async_trait, Error, RpcResult};
use katana_executor::{EntryPointCall, ExecutorFactory};
use katana_primitives::block::BlockIdOrTag;
use katana_primitives::class::ClassHash;
use katana_primitives::genesis::allocation::GenesisAccountAlloc;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxHash};
use katana_primitives::{ContractAddress, Felt};
use katana_provider::traits::state::StateFactoryProvider;
use katana_rpc_api::starknet::StarknetApiServer;
use katana_rpc_types::block::{
    BlockHashAndNumber, MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes,
    MaybePendingBlockWithTxs,
};
use katana_rpc_types::class::RpcContractClass;
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::event::{EventFilterWithPage, EventsPage};
use katana_rpc_types::message::MsgFromL1;
use katana_rpc_types::receipt::TxReceiptWithBlockInfo;
use katana_rpc_types::state_update::MaybePendingStateUpdate;
use katana_rpc_types::transaction::{BroadcastedTx, Tx};
use katana_rpc_types::trie::{ContractStorageKeys, GetStorageProofResponse};
use katana_rpc_types::{FeeEstimate, FeltAsHex, FunctionCall, SimulationFlagForEstimateFee};
use starknet::core::types::TransactionStatus;

use super::StarknetApi;
use crate::cartridge;

#[async_trait]
impl<EF: ExecutorFactory> StarknetApiServer for StarknetApi<EF> {
    async fn chain_id(&self) -> RpcResult<FeltAsHex> {
        Ok(self.inner.backend.chain_spec.id().id().into())
    }

    async fn get_nonce(
        &self,
        block_id: BlockIdOrTag,
        contract_address: Felt,
    ) -> RpcResult<FeltAsHex> {
        Ok(self.nonce_at(block_id, contract_address.into()).await?.into())
    }

    async fn block_number(&self) -> RpcResult<u64> {
        Ok(self.latest_block_number().await?)
    }

    async fn get_transaction_by_hash(&self, transaction_hash: Felt) -> RpcResult<Tx> {
        Ok(self.transaction(transaction_hash).await?)
    }

    async fn get_block_transaction_count(&self, block_id: BlockIdOrTag) -> RpcResult<u64> {
        Ok(self.block_tx_count(block_id).await?)
    }

    async fn get_class_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: Felt,
    ) -> RpcResult<RpcContractClass> {
        Ok(self.class_at_address(block_id, contract_address.into()).await?)
    }

    async fn block_hash_and_number(&self) -> RpcResult<BlockHashAndNumber> {
        self.on_io_blocking_task(move |this| {
            let res = this.block_hash_and_number()?;
            Ok(res.into())
        })
        .await
    }

    async fn get_block_with_tx_hashes(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<MaybePendingBlockWithTxHashes> {
        Ok(self.block_with_tx_hashes(block_id).await?)
    }

    async fn get_transaction_by_block_id_and_index(
        &self,
        block_id: BlockIdOrTag,
        index: u64,
    ) -> RpcResult<Tx> {
        Ok(self.transaction_by_block_id_and_index(block_id, index).await?)
    }

    async fn get_block_with_txs(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<MaybePendingBlockWithTxs> {
        Ok(self.block_with_txs(block_id).await?)
    }

    async fn get_block_with_receipts(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<MaybePendingBlockWithReceipts> {
        Ok(self.block_with_receipts(block_id).await?)
    }

    async fn get_state_update(&self, block_id: BlockIdOrTag) -> RpcResult<MaybePendingStateUpdate> {
        Ok(self.state_update(block_id).await?)
    }

    async fn get_transaction_receipt(
        &self,
        transaction_hash: Felt,
    ) -> RpcResult<TxReceiptWithBlockInfo> {
        Ok(self.receipt(transaction_hash).await?)
    }

    async fn get_class_hash_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: Felt,
    ) -> RpcResult<FeltAsHex> {
        Ok(self.class_hash_at_address(block_id, contract_address.into()).await?.into())
    }

    async fn get_class(
        &self,
        block_id: BlockIdOrTag,
        class_hash: Felt,
    ) -> RpcResult<RpcContractClass> {
        Ok(self.class_at_hash(block_id, class_hash).await?)
    }

    async fn get_events(&self, filter: EventFilterWithPage) -> RpcResult<EventsPage> {
        Ok(self.events(filter).await?)
    }

    async fn call(
        &self,
        request: FunctionCall,
        block_id: BlockIdOrTag,
    ) -> RpcResult<Vec<FeltAsHex>> {
        self.on_io_blocking_task(move |this| {
            let request = EntryPointCall {
                calldata: request.calldata,
                contract_address: request.contract_address.into(),
                entry_point_selector: request.entry_point_selector,
            };

            // get the state and block env at the specified block for function call execution
            let state = this.state(&block_id)?;
            let env = this.block_env_at(&block_id)?;
            let executor = this.inner.backend.executor_factory.with_state_and_block_env(state, env);

            match executor.call(request) {
                Ok(retdata) => Ok(retdata.into_iter().map(|v| v.into()).collect()),
                Err(err) => Err(Error::from(StarknetApiError::ContractError {
                    revert_error: err.to_string(),
                })),
            }
        })
        .await
    }

    async fn get_storage_at(
        &self,
        contract_address: Felt,
        key: Felt,
        block_id: BlockIdOrTag,
    ) -> RpcResult<FeltAsHex> {
        self.on_io_blocking_task(move |this| {
            let value = this.storage_at(contract_address.into(), key, block_id)?;
            Ok(value.into())
        })
        .await
    }

    async fn estimate_fee(
        &self,
        request: Vec<BroadcastedTx>,
        simulation_flags: Vec<SimulationFlagForEstimateFee>,
        block_id: BlockIdOrTag,
    ) -> RpcResult<Vec<FeeEstimate>> {
        let chain_id = self.inner.backend.chain_spec.id();

        let transactions = request
            .into_iter()
            .map(|tx| {
                let tx = match tx {
                    BroadcastedTx::Invoke(tx) => {
                        let is_query = tx.is_query();
                        let tx = tx.into_tx_with_chain_id(chain_id);
                        ExecutableTxWithHash::new_query(ExecutableTx::Invoke(tx), is_query)
                    }

                    BroadcastedTx::DeployAccount(tx) => {
                        let is_query = tx.is_query();
                        let tx = tx.into_tx_with_chain_id(chain_id);
                        ExecutableTxWithHash::new_query(ExecutableTx::DeployAccount(tx), is_query)
                    }

                    BroadcastedTx::Declare(tx) => {
                        let is_query = tx.is_query();
                        let tx = tx
                            .try_into_tx_with_chain_id(chain_id)
                            .map_err(|_| StarknetApiError::InvalidContractClass)?;
                        ExecutableTxWithHash::new_query(ExecutableTx::Declare(tx), is_query)
                    }
                };

                Result::<ExecutableTxWithHash, StarknetApiError>::Ok(tx)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let skip_validate = simulation_flags.contains(&SimulationFlagForEstimateFee::SkipValidate);

        // If the node is run with transaction validation disabled, then we should not validate
        // transactions when estimating the fee even if the `SKIP_VALIDATE` flag is not set.
        let should_validate = !skip_validate
            && self.inner.backend.executor_factory.execution_flags().account_validation();

        // We don't care about the nonce when estimating the fee as the nonce value
        // doesn't affect transaction execution.
        //
        // This doesn't completely disregard the nonce as nonce < account nonce will
        // return an error. It only 'relaxes' the check for nonce >= account nonce.
        let flags = katana_executor::ExecutionFlags::new()
            .with_account_validation(should_validate)
            .with_nonce_check(false);

        // Hook the estimate fee to pre-deploy the controller contract
        // and enhance UX on the client side.
        // Refer to the `handle_cartridge_controller_deploy` function in `cartridge.rs`
        // for more details.
        #[cfg(feature = "cartridge")]
        let transactions = if let Some(paymaster) = &self.inner.config.paymaster {
            // Paymaster is the first dev account in the genesis.
            let (paymaster_address, paymaster_alloc) = self
                .inner
                .backend
                .chain_spec
                .genesis()
                .accounts()
                .nth(0)
                .ok_or(anyhow!("Cartridge paymaster account doesn't exist"))?;

            let paymaster_private_key = if let GenesisAccountAlloc::DevAccount(pm) = paymaster_alloc
            {
                pm.private_key
            } else {
                let reason = "Paymaster is not a dev account".to_string();
                return Err(StarknetApiError::UnexpectedError { reason }.into());
            };

            let state = self
                .inner
                .backend
                .blockchain
                .provider()
                .latest()
                .map(Arc::new)
                .map_err(StarknetApiError::from)?;

            let mut ctrl_deploy_txs = Vec::new();

            // Check if any of the transactions are sent from an address associated with a Cartridge
            // Controller account. If yes, we craft a Controller deployment transaction
            // for each of the unique sender and push it at the beginning of the
            // transaction list so that all the requested transactions are executed against a state
            // with the Controller accounts deployed.

            let paymaster_nonce = match self.nonce_at(block_id, *paymaster_address).await {
                Ok(nonce) => nonce,
                Err(err) => match err {
                    // this should be unreachable bcs we already checked for the paymaster account
                    // existence earlier
                    StarknetApiError::ContractNotFound => {
                        let error = anyhow!("Cartridge paymaster account doesn't exist");
                        return Err(Error::from(error))?;
                    }
                    _ => return Err(Error::from(err)),
                },
            };

            for tx in &transactions {
                let deploy_controller_tx =
                    cartridge::get_controller_deploy_tx_if_controller_address(
                        *paymaster_address,
                        paymaster_private_key,
                        paymaster_nonce,
                        tx,
                        self.inner.backend.chain_spec.id(),
                        state.clone(),
                        &paymaster.cartridge_api_url,
                    )
                    .await?;

                if let Some(tx) = deploy_controller_tx {
                    ctrl_deploy_txs.push(tx);
                }
            }

            if !ctrl_deploy_txs.is_empty() {
                ctrl_deploy_txs.extend(transactions);
                ctrl_deploy_txs
            } else {
                transactions
            }
        } else {
            transactions
        };

        self.on_cpu_blocking_task(move |this| {
            let results = this.estimate_fee_with(transactions, block_id, flags)?;
            Ok(results)
        })
        .await
    }

    async fn estimate_message_fee(
        &self,
        message: MsgFromL1,
        block_id: BlockIdOrTag,
    ) -> RpcResult<FeeEstimate> {
        self.on_cpu_blocking_task(move |this| {
            let chain_id = this.inner.backend.chain_spec.id();

            let tx = message.into_tx_with_chain_id(chain_id);
            let hash = tx.calculate_hash();

            let result = this.estimate_fee_with(
                vec![ExecutableTxWithHash { hash, transaction: tx.into() }],
                block_id,
                Default::default(),
            );
            match result {
                Ok(mut res) => {
                    if let Some(fee) = res.pop() {
                        Ok(fee)
                    } else {
                        Err(Error::from(StarknetApiError::UnexpectedError {
                            reason: "Fee estimation result should exist".into(),
                        }))
                    }
                }

                Err(err) => Err(Error::from(err)),
            }
        })
        .await
    }

    async fn get_transaction_status(
        &self,
        transaction_hash: TxHash,
    ) -> RpcResult<TransactionStatus> {
        Ok(self.transaction_status(transaction_hash).await?)
    }

    async fn get_storage_proof(
        &self,
        block_id: BlockIdOrTag,
        class_hashes: Option<Vec<ClassHash>>,
        contract_addresses: Option<Vec<ContractAddress>>,
        contracts_storage_keys: Option<Vec<ContractStorageKeys>>,
    ) -> RpcResult<GetStorageProofResponse> {
        let proofs = self
            .get_proofs(block_id, class_hashes, contract_addresses, contracts_storage_keys)
            .await?;
        Ok(proofs)
    }
}
