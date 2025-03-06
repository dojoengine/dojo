use std::sync::Arc;

use account_sdk::account::outside_execution::OutsideExecution;
use account_sdk::signers::Signer;
use cainome::cairo_serde::CairoSerde;
use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::backend::Backend;
use katana_core::service::block_producer::BlockProducer;
use katana_executor::ExecutorFactory;
use katana_pool::{TransactionPool, TxPool};
use katana_primitives::da::DataAvailabilityMode;
use katana_primitives::fee::ResourceBoundsMapping;
use katana_primitives::genesis::allocation::GenesisAccountAlloc;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, InvokeTx, InvokeTxV3};
use katana_primitives::{ContractAddress, Felt};
use katana_provider::traits::state::StateFactoryProvider;
use katana_rpc_api::cartridge::CartridgeApiServer;
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::transaction::InvokeTxResult;
use katana_tasks::TokioTaskSpawner;
use starknet::core::types::Call;
use starknet::macros::{selector, short_string};
use starknet::signers::{LocalWallet, Signer as SnSigner, SigningKey};

pub const GUARDIAN_SIGNER: Signer =
    Signer::Starknet(SigningKey::from_secret_scalar(short_string!("CARTRIDGE_GUARDIAN")));

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
        Self {
            backend: Arc::clone(&self.backend),
            block_producer: self.block_producer.clone(),
            pool: self.pool.clone(),
        }
    }
}

impl<EF: ExecutorFactory> CartridgeApi<EF> {
    pub fn new(backend: Arc<Backend<EF>>, block_producer: BlockProducer<EF>, pool: TxPool) -> Self {
        Self { backend, block_producer, pool }
    }

    pub async fn execute_outside(
        &self,
        paymaster_address: ContractAddress,
        contract_address: ContractAddress,
        outside_execution: OutsideExecution,
        signature: Vec<Felt>,
    ) -> Result<InvokeTxResult, StarknetApiError> {

        let (_, paymaster_alloc) = self
            .backend
            .chain_spec
            .genesis()
            .accounts()
            .last()
            .unwrap();
        
        let private_key = if let GenesisAccountAlloc::DevAccount(pm) = paymaster_alloc {
            pm.private_key
        } else {
            panic!("Paymaster is not a dev account");
        };

        let state = self.backend.blockchain.provider().latest().unwrap();
        let nonce = state.nonce(paymaster_address).unwrap();

        let entrypoint = match outside_execution {
            OutsideExecution::V2(_) => selector!("execute_from_outside_v2"),
            OutsideExecution::V3(_) => selector!("execute_from_outside_v3"),
        };

        let mut inner_calldata = <OutsideExecution as CairoSerde>::cairo_serialize(&outside_execution);
        inner_calldata.extend(<Vec<Felt> as CairoSerde>::cairo_serialize(&signature));

        let call = Call {
            to: contract_address.into(),
            selector: entrypoint,
            calldata: inner_calldata,
        };

        self.on_io_blocking_task(move |this| {
            let mut tx = InvokeTxV3 {
                chain_id: this.backend.chain_spec.id(),
                nonce: nonce.unwrap_or(Felt::ZERO),
                calldata: this.encode_calls(vec![call]),
                signature: vec![],
                sender_address: paymaster_address.into(),
                resource_bounds: ResourceBoundsMapping::default(),
                tip: 0_u64,
                paymaster_data: vec![],
                account_deployment_data: vec![],
                nonce_data_availability_mode: DataAvailabilityMode::L1,
                fee_data_availability_mode: DataAvailabilityMode::L1,
            };
            let tx_hash = InvokeTx::V3(tx.clone()).calculate_hash(false);

            let signer = LocalWallet::from(SigningKey::from_secret_scalar(
                private_key,
            ));

            let signature = futures::executor::block_on(signer.sign_hash(&tx_hash)).expect("failed to sign hash with paymaster");
            tx.signature = vec![signature.r, signature.s];

            let tx = ExecutableTxWithHash::new(ExecutableTx::Invoke(InvokeTx::V3(tx)));
            let hash = this.pool.add_transaction(tx)?;
            Ok(InvokeTxResult::new(hash))
        })
        .await
    }

    async fn on_io_blocking_task<F, T>(&self, func: F) -> T
    where
        F: FnOnce(Self) -> T + Send + 'static,
        T: Send + 'static,
    {
        let this = self.clone();
        TokioTaskSpawner::new().unwrap().spawn_blocking(move || func(this)).await.unwrap()
    }

    fn encode_calls(&self, calls: Vec<Call>) -> Vec<Felt> {
        let mut execute_calldata: Vec<Felt> = vec![calls.len().into()];
                for call in calls {
                    execute_calldata.push(call.to); // to
                    execute_calldata.push(call.selector); // selector

                    execute_calldata.push(call.calldata.len().into()); // calldata.len()
                    execute_calldata.extend_from_slice(&call.calldata);
                }

        execute_calldata
    }
}

#[async_trait]
impl<EF: ExecutorFactory> CartridgeApiServer for CartridgeApi<EF> {
    async fn add_execute_outside_transaction(
        &self,
        address: ContractAddress,
        outside_execution: OutsideExecution,
        signature: Vec<Felt>,
    ) -> RpcResult<InvokeTxResult> {
        let (addr, _) = self.backend.chain_spec.genesis().accounts().last().unwrap();

        Ok(self.execute_outside(addr.clone(), address, outside_execution, signature).await?)
    }
}
