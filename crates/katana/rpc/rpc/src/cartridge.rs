use std::sync::Arc;

use account_sdk::abigen::controller::{SessionToken, SignerSignature};
use account_sdk::account::outside_execution::OutsideExecution;
use account_sdk::account::session::hash::SessionHash;
use account_sdk::hash::MessageHashRev1;
use account_sdk::signers::{HashSigner, Signer};
use cainome::cairo_serde::CairoSerde;
use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::backend::Backend;
use katana_core::service::block_producer::BlockProducer;
use katana_executor::ExecutorFactory;
use katana_pool::{TransactionPool, TxPool};
use katana_primitives::da::DataAvailabilityMode;
use katana_primitives::fee::ResourceBoundsMapping;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, InvokeTx, InvokeTxV3};
use katana_primitives::{ContractAddress, Felt};
use katana_provider::traits::state::StateFactoryProvider;
use katana_rpc_api::cartridge::CartridgeApiServer;
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::transaction::InvokeTxResult;
use katana_tasks::TokioTaskSpawner;
use starknet::core::types::Call;
use starknet::macros::{selector, short_string};
use starknet::signers::SigningKey;

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

        let state = self.backend.blockchain.provider().latest().unwrap();
        let nonce = state.nonce(paymaster_address).unwrap();

        let hash = match &outside_execution {
            OutsideExecution::V2(v2) => {
                v2.get_message_hash_rev_1(self.backend.chain_spec.id().into(), contract_address.into())
            }
            OutsideExecution::V3(v3) => {
                v3.get_message_hash_rev_1(self.backend.chain_spec.id().into(), contract_address.into())
            }
        };

        let signature =
            self.add_guardian_signature(contract_address.into(), hash, signature.as_slice()).await;

        let mut inner_calldata = <OutsideExecution as CairoSerde>::cairo_serialize(&outside_execution);
        inner_calldata.extend(<Vec<Felt> as CairoSerde>::cairo_serialize(&signature));


        let call = Call {
            to: contract_address.into(),
            selector: selector!("execute_from_outside_v3"),
            calldata: inner_calldata,
        };

        self.on_io_blocking_task(move |this| {
            let tx = InvokeTx::V3(InvokeTxV3 {
                chain_id: this.backend.chain_spec.id(),
                nonce: nonce.unwrap_or(Felt::ZERO),
                calldata: this.encode_calls(vec![call]),
                signature: signature.clone(),
                sender_address: paymaster_address.into(),
                resource_bounds: ResourceBoundsMapping::default(),
                tip: 0_u64,
                paymaster_data: vec![],
                account_deployment_data: vec![],
                nonce_data_availability_mode: DataAvailabilityMode::L1,
                fee_data_availability_mode: DataAvailabilityMode::L1,
            });
            let tx = ExecutableTxWithHash::new(ExecutableTx::Invoke(tx));
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

    async fn add_guardian_signature(
        &self,
        address: Felt,
        tx_hash: Felt,
        old_signature: &[Felt],
    ) -> Vec<Felt> {
        match <Vec<SignerSignature> as CairoSerde>::cairo_deserialize(old_signature, 0) {
            Ok(mut signature) => {
                let guardian_signature = GUARDIAN_SIGNER.sign(&tx_hash).await.unwrap();
                signature.push(guardian_signature);
                <Vec<SignerSignature> as CairoSerde>::cairo_serialize(&signature)
            }
            Err(_) => {
                let mut session_token =
                    <SessionToken as CairoSerde>::cairo_deserialize(old_signature, 1).unwrap();
                let session_token_hash = session_token
                    .session
                    .hash(self.backend.chain_spec.id().into(), address, tx_hash)
                    .unwrap();

                // This is different from the transaction signature
                self.add_guardian_authorization(&mut session_token, address).await;

                let guardian_signature = GUARDIAN_SIGNER.sign(&session_token_hash).await.unwrap();
                session_token.guardian_signature = guardian_signature;

                let mut serialized = <SessionToken as CairoSerde>::cairo_serialize(&session_token);
                serialized.insert(0, old_signature[0]);
                serialized
            }
        }
    }

    async fn add_guardian_authorization(&self, session_token: &mut SessionToken, address: Felt) {
        if session_token.session_authorization.len() == 2 {
            // Authorization by registered
            return;
        }
        let authorization = <Vec<SignerSignature> as CairoSerde>::cairo_deserialize(
            &session_token.session_authorization,
            0,
        )
        .unwrap();
        if authorization.len() == 1 {
            let session_hash = session_token
                .session
                .get_message_hash_rev_1(self.backend.chain_spec.id().into(), address);
            let guardian_authorization = GUARDIAN_SIGNER.sign(&session_hash).await.unwrap();
            session_token.session_authorization =
                <Vec<SignerSignature> as CairoSerde>::cairo_serialize(&vec![
                    authorization[0].clone(),
                    guardian_authorization,
                ]);
        }
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
        let (addr, alloc) = self.backend.chain_spec.genesis().accounts().last().unwrap();

        Ok(self.execute_outside(addr.clone(), address, outside_execution, signature).await?)
    }
}
