use core::panic;
use std::sync::Arc;

use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::backend::Backend;
use katana_core::service::block_producer::{BlockProducer, BlockProducerMode, PendingExecutor};
use katana_executor::ExecutorFactory;
use katana_pool::{TxPool, TransactionPool};
use katana_primitives::{ContractAddress, Felt};
use katana_primitives::transaction::{ExecutableTx,ExecutableTxWithHash, InvokeTx, InvokeTxV1};
use katana_rpc_api::dev::DevApiServer;
use katana_rpc_api::cartridge::CartridgeApiServer;
use katana_rpc_types::error::{dev::DevApiError, starknet::StarknetApiError};
use katana_rpc_types::transaction::{ExecuteOutside, InvokeTxResult};
use katana_tasks::TokioTaskSpawner;
use starknet::core::types::InvokeTransactionResult;
use starknet::core::types::{BlockId, BlockTag};
use starknet::accounts::SingleOwnerAccount;

#[allow(missing_debug_implementations)]
pub struct CartridgeApi<EF: ExecutorFactory> {
    backend: Arc<Backend<EF>>,
    block_producer: BlockProducer<EF>,
    pool: TxPool,
}

impl<EF> Clone for CartridgeApi<EF>
where
    EF: ExecutorFactory,
{
    fn clone(&self) -> Self {
        Self { backend: Arc::clone(&self.backend), block_producer: self.block_producer.clone(), pool: self.pool.clone() }
    }
}

impl<EF: ExecutorFactory> CartridgeApi<EF> {
    pub fn new(backend: Arc<Backend<EF>>, block_producer: BlockProducer<EF>, pool: TxPool) -> Self {
        Self { backend, block_producer, pool }
    }
    pub async fn execute_outside(&self, address: ContractAddress, nonce: Option<Felt>,outside_execution: ExecuteOutside, signature: Vec<Felt>) -> Result<InvokeTxResult, StarknetApiError> {
        self.on_io_blocking_task(move |this| {
            let tx = match outside_execution {
                ExecuteOutside::V2(v2) => InvokeTx::V1(InvokeTxV1 {
                    chain_id: this.backend.chain_spec.id(),
                    nonce: nonce.unwrap_or(Felt::ZERO),
                    calldata: v2.calls[0].calldata.clone(),
                    signature: signature,
                    sender_address: address.into(),
                    max_fee: 0,
                }),
                ExecuteOutside::V3(v3) => InvokeTx::V1(InvokeTxV1 {
                    chain_id: this.backend.chain_spec.id(),
                    nonce: nonce.unwrap_or(Felt::ZERO),
                    calldata: v3.calls[0].calldata.clone(),
                    signature: signature,
                    sender_address: address.into(),
                    max_fee: 0,
                }),
            };
            let tx = ExecutableTxWithHash::new(ExecutableTx::Invoke(tx));
            let hash = this.pool.add_transaction(tx)?;
            Ok(InvokeTxResult::new(hash))
        }).await
    }

    async fn on_io_blocking_task<F, T>(&self, func: F) -> T
    where
        F: FnOnce(Self) -> T + Send + 'static,
        T: Send + 'static,
    {
        let this = self.clone();
        TokioTaskSpawner::new().unwrap().spawn_blocking(move || func(this)).await.unwrap()
    }
}


#[async_trait]
impl<EF: ExecutorFactory> CartridgeApiServer for CartridgeApi<EF> {
    async fn add_execute_outside_transaction(&self, address: ContractAddress, outside_execution: ExecuteOutside, signature: Vec<Felt>) -> RpcResult<InvokeTxResult> {
        let (addr, alloc) = self.backend.chain_spec.genesis().accounts().take(1).next().unwrap();
        println!(
            "{:#?}", outside_execution
        );

        println!(
            "{:#?}\n\n{:#?}", addr, alloc
        );

            // let accounts = DevAllocationsGenerator::new(self.development.total_accounts)
            //     .with_seed(parse_seed(&self.development.seed))
            //     .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
            //     .with_class(katana_primitives::felt!("0x024a9edbfa7082accfceabf6a92d7160086f346d622f28741bf1c651c412c9ab"))
            //     .generate();
        // self.backend.executor_factory
        // let mut account = SingleOwnerAccount::new(addr.clone(), alloc.balance().unwrap());
        // account.set_block_id(BlockId::Tag(BlockTag::Pending));

        Ok(self.execute_outside(addr.clone(), alloc.nonce(), outside_execution, signature).await?)
    }
}