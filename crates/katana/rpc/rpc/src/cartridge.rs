use std::sync::Arc;

use account_sdk::account::outside_execution::OutsideExecution;
use cainome::cairo_serde::CairoSerde;
use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::backend::Backend;
use katana_core::service::block_producer::BlockProducer;
use katana_executor::ExecutorFactory;
use katana_pool::{TransactionPool, TxPool};
use katana_primitives::chain::ChainId;
use katana_primitives::da::DataAvailabilityMode;
use katana_primitives::fee::ResourceBoundsMapping;
use katana_primitives::genesis::allocation::GenesisAccountAlloc;
use katana_primitives::genesis::constant::DEFAULT_UDC_ADDRESS;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, InvokeTx, InvokeTxV3};
use katana_primitives::{ContractAddress, Felt};
use katana_provider::traits::state::{StateFactoryProvider, StateProvider};
use katana_rpc_api::cartridge::CartridgeApiServer;
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::transaction::InvokeTxResult;
use katana_rpc_types::FeeEstimate;
use katana_tasks::TokioTaskSpawner;
use serde::Deserialize;
use starknet::core::types::{Call, PriceUnit};
use starknet::macros::selector;
use starknet::signers::{LocalWallet, Signer, SigningKey};

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
        paymaster_private_key: Felt,
        contract_address: ContractAddress,
        outside_execution: OutsideExecution,
        signature: Vec<Felt>,
    ) -> Result<InvokeTxResult, StarknetApiError> {
        let state = self.backend.blockchain.provider().latest().unwrap();
        let nonce = state.nonce(paymaster_address).unwrap();

        let entrypoint = match outside_execution {
            OutsideExecution::V2(_) => selector!("execute_from_outside_v2"),
            OutsideExecution::V3(_) => selector!("execute_from_outside_v3"),
        };

        let mut inner_calldata =
            <OutsideExecution as CairoSerde>::cairo_serialize(&outside_execution);
        inner_calldata.extend(<Vec<Felt> as CairoSerde>::cairo_serialize(&signature));

        let call =
            Call { to: contract_address.into(), selector: entrypoint, calldata: inner_calldata };

        self.on_io_blocking_task(move |this| {
            let mut tx = InvokeTxV3 {
                chain_id: this.backend.chain_spec.id(),
                nonce: nonce.unwrap_or(Felt::ZERO),
                calldata: encode_calls(vec![call]),
                signature: vec![],
                sender_address: paymaster_address,
                resource_bounds: ResourceBoundsMapping::default(),
                tip: 0_u64,
                paymaster_data: vec![],
                account_deployment_data: vec![],
                nonce_data_availability_mode: DataAvailabilityMode::L1,
                fee_data_availability_mode: DataAvailabilityMode::L1,
            };
            let tx_hash = InvokeTx::V3(tx.clone()).calculate_hash(false);

            let signer = LocalWallet::from(SigningKey::from_secret_scalar(paymaster_private_key));

            let signature = futures::executor::block_on(signer.sign_hash(&tx_hash))
                .expect("failed to sign hash with paymaster");
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
}

#[async_trait]
impl<EF: ExecutorFactory> CartridgeApiServer for CartridgeApi<EF> {
    async fn add_execute_outside_transaction(
        &self,
        address: ContractAddress,
        outside_execution: OutsideExecution,
        signature: Vec<Felt>,
    ) -> RpcResult<InvokeTxResult> {
        let (paymaster_address, paymaster_alloc) =
            self.backend.chain_spec.genesis().accounts().nth(0).unwrap();

        let paymaster_private_key = if let GenesisAccountAlloc::DevAccount(pm) = paymaster_alloc {
            pm.private_key
        } else {
            panic!("Paymaster is not a dev account");
        };

        Ok(self
            .execute_outside(
                *paymaster_address,
                paymaster_private_key,
                address,
                outside_execution,
                signature,
            )
            .await?)
    }
}

/// Response from the Cartridge API to fetch the calldata for the constructor of the given
/// controller address.
#[derive(Debug, Deserialize)]
#[allow(unused)]
struct CartridgeAccountResponse {
    /// The address of the controller account.
    pub address: Felt,
    /// The username of the controller account used as salt.
    pub username: String,
    /// The calldata for the constructor of the given controller address, this is
    /// UDC calldata, already containing the class hash and the salt + the constructor arguments.
    pub calldata: Vec<Felt>,
}

/// Calls the Cartridge API to fetch the calldata for the constructor of the given controller
/// address.
async fn fetch_controller_constructor_calldata(address: Felt) -> Option<Vec<Felt>> {
    // This URL is used to fetch the calldata for the constructor of the given controller address.
    // Will return 404 if the controller address is not found.
    const CARTRIDGE_ACCOUNTS_CALLDATA_URL: &str = "https://api.cartridge.gg/accounts/calldata";

    let body = serde_json::json!({
        "address": format!("{:#066x}", address)
    });

    let client = reqwest::Client::new();
    let response = client
        .post(CARTRIDGE_ACCOUNTS_CALLDATA_URL)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .ok()?;

    let response: CartridgeAccountResponse = if let Ok(r) = response.json().await {
        r
    } else {
        return None;
    };

    Some(response.calldata)
}

/// Encodes the given calls into a vector of Felt values (New encoding, cairo 1),
/// since controller accounts are Cairo 1 contracts.
pub fn encode_calls(calls: Vec<Call>) -> Vec<Felt> {
    let mut execute_calldata: Vec<Felt> = vec![calls.len().into()];
    for call in calls {
        execute_calldata.push(call.to);
        execute_calldata.push(call.selector);

        execute_calldata.push(call.calldata.len().into());
        execute_calldata.extend_from_slice(&call.calldata);
    }

    execute_calldata
}

/// Handles the deployment of a cartridge controller if the estimate fee is requested for a
/// cartridge controller.
///
/// The controller accounts are created with a specific version of the controller.
/// To ensure address determinism, the controller account must be deployed with the same version,
/// which is included in the calldata retrieved from the Cartridge API.
pub async fn handle_cartridge_controller_deploy(
    paymaster_address: ContractAddress,
    paymaster_private_key: Felt,
    transactions: &[ExecutableTxWithHash],
    chain_id: ChainId,
    state: Box<dyn StateProvider>,
) -> Option<(ExecutableTxWithHash, FeeEstimate)> {
    let paymaster_nonce = state.nonce(paymaster_address).expect("failed to get paymaster nonce");

    for t in transactions {
        if let ExecutableTx::Invoke(InvokeTx::V3(v3)) = &t.transaction {
            let maybe_controller_address: Felt = v3.sender_address.into();

            // Avoid deploying the controller account if it is already deployed.
            if state.class_hash_of_contract(maybe_controller_address.into()).unwrap().is_some() {
                return None;
            }

            let calldata = fetch_controller_constructor_calldata(maybe_controller_address).await?;

            let call = Call {
                to: DEFAULT_UDC_ADDRESS.into(),
                selector: selector!("deployContract"),
                calldata,
            };

            let mut tx = InvokeTxV3 {
                chain_id,
                nonce: paymaster_nonce.unwrap_or(Felt::ZERO),
                calldata: encode_calls(vec![call]),
                sender_address: paymaster_address,
                resource_bounds: ResourceBoundsMapping::default(),
                tip: 0_u64,
                paymaster_data: vec![],
                account_deployment_data: vec![],
                nonce_data_availability_mode: katana_primitives::da::DataAvailabilityMode::L1,
                fee_data_availability_mode: katana_primitives::da::DataAvailabilityMode::L1,
                signature: vec![],
            };
            let tx_hash = InvokeTx::V3(tx.clone()).calculate_hash(false);

            let signer = LocalWallet::from(SigningKey::from_secret_scalar(paymaster_private_key));

            let signature = futures::executor::block_on(signer.sign_hash(&tx_hash))
                .expect("failed to sign hash with paymaster");
            tx.signature = vec![signature.r, signature.s];

            let tx = ExecutableTxWithHash::new(ExecutableTx::Invoke(InvokeTx::V3(tx)));

            return Some((
                tx,
                FeeEstimate {
                    gas_price: Felt::ZERO,
                    gas_consumed: Felt::ZERO,
                    overall_fee: Felt::ZERO,
                    data_gas_price: Default::default(),
                    data_gas_consumed: Default::default(),
                    unit: PriceUnit::Fri,
                },
            ));
        }
    }

    None
}
