//! Handles management of Cartridge controller accounts.
//!
//! When a Controller account is created, the username is used as a salt,
//! and the latest controller class hash is used.
//! This ensures that the controller account address is deterministic.
//!
//! A consequence of that, is that all the controller class hashes must be
//! known by Katana to ensure it can first deploy the controller account with the
//! correct address, and then upgrade it to the latest version.
//!
//! This module contains the function to work around this behavior, which also relies
//! on the updated code into `katana-primitives` to ensure all the controller class hashes
//! are available.
//!
//! Two flows:
//!
//! 1. When a Controller account is created, an execution from outside is received from the very
//!    first transaction that the user will want to achieve using the session. In this case, this
//!    module will hook the execution from outside to ensure the controller account is deployed.
//!
//! 2. When a Controller account is already deployed, and the user logs in, the client code of
//!    controller is actually performing a `estimate_fee` to estimate the fee for the account
//!    upgrade. In this case, this module contains the code to hook the fee estimation, and return
//!    the associated transaction to be executed in order to deploy the controller account.

use std::sync::Arc;

use account_sdk::account::outside_execution::OutsideExecution;
use anyhow::anyhow;
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
use katana_tasks::TokioTaskSpawner;
use serde::Deserialize;
use starknet::core::types::Call;
use starknet::macros::selector;
use starknet::signers::{LocalWallet, Signer, SigningKey};
use tracing::debug;
use url::Url;

#[allow(missing_debug_implementations)]
pub struct CartridgeApi<EF: ExecutorFactory> {
    backend: Arc<Backend<EF>>,
    block_producer: BlockProducer<EF>,
    pool: TxPool,
    /// The root URL for the Cartridge API for paymaster related operations.
    api_url: Url,
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
            api_url: self.api_url.clone(),
        }
    }
}

impl<EF: ExecutorFactory> CartridgeApi<EF> {
    pub fn new(
        backend: Arc<Backend<EF>>,
        block_producer: BlockProducer<EF>,
        pool: TxPool,
        api_url: Url,
    ) -> Self {
        Self { backend, block_producer, pool, api_url }
    }

    pub async fn execute_outside(
        &self,
        address: ContractAddress,
        outside_execution: OutsideExecution,
        signature: Vec<Felt>,
    ) -> Result<InvokeTxResult, StarknetApiError> {
        debug!(%address, ?outside_execution, "Adding execute outside transaction.");
        self.on_io_blocking_task(move |this| {
            let (pm_address, pm_acc) = this
                .backend
                .chain_spec
                .genesis()
                .accounts()
                .nth(0)
                .ok_or(anyhow!("Cartridge paymaster account doesn't exist"))?;

            let pm_private_key = if let GenesisAccountAlloc::DevAccount(pm) = pm_acc {
                pm.private_key
            } else {
                panic!("Paymaster is not a dev account");
            };

            let provider = this.backend.blockchain.provider();
            let state = provider.latest()?;

            let entrypoint = match outside_execution {
                OutsideExecution::V2(_) => selector!("execute_from_outside_v2"),
                OutsideExecution::V3(_) => selector!("execute_from_outside_v3"),
            };

            // If the controller has been created during the flow, there's no fee estimation.
            // Hence, we can check if the controller is deployed, if not, deploy it.
            if state.class_hash_of_contract(address)?.is_none() {
                let nonce = state.nonce(*pm_address)?;
                if let Some(tx) =
                    futures::executor::block_on(craft_deploy_cartridge_controller_tx(
                        &this.api_url,
                        address,
                        *pm_address,
                        pm_private_key,
                        this.backend.chain_spec.id(),
                        nonce,
                    ))?
                {
                    this.pool.add_transaction(tx)?;
                }
            }

            let nonce = state.nonce(*pm_address)?;

            let mut inner_calldata =
                <OutsideExecution as CairoSerde>::cairo_serialize(&outside_execution);
            inner_calldata.extend(<Vec<Felt> as CairoSerde>::cairo_serialize(&signature));

            let call = Call { to: address.into(), selector: entrypoint, calldata: inner_calldata };

            let mut tx = InvokeTxV3 {
                chain_id: this.backend.chain_spec.id(),
                nonce: nonce.unwrap_or(Felt::ZERO),
                calldata: encode_calls(vec![call]),
                signature: vec![],
                sender_address: *pm_address,
                resource_bounds: ResourceBoundsMapping::default(),
                tip: 0_u64,
                paymaster_data: vec![],
                account_deployment_data: vec![],
                nonce_data_availability_mode: DataAvailabilityMode::L1,
                fee_data_availability_mode: DataAvailabilityMode::L1,
            };
            let tx_hash = InvokeTx::V3(tx.clone()).calculate_hash(false);

            let signer = LocalWallet::from(SigningKey::from_secret_scalar(pm_private_key));
            let signature =
                futures::executor::block_on(signer.sign_hash(&tx_hash)).map_err(|e| anyhow!(e))?;
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
        Ok(self.execute_outside(address, outside_execution, signature).await?)
    }
}

/// Response from the Cartridge API to fetch the calldata for the constructor of the given
/// controller address.
#[derive(Debug, Deserialize)]
struct CartridgeAccountResponse {
    /// The address of the controller account.
    #[allow(unused)]
    address: Felt,
    /// The username of the controller account used as salt.
    #[allow(unused)]
    username: String,
    /// The calldata for the constructor of the given controller address, this is
    /// UDC calldata, already containing the class hash and the salt + the constructor arguments.
    calldata: Vec<Felt>,
}

/// Calls the Cartridge API to fetch the calldata for the constructor of the given controller
/// address.
///
/// Returns None if the controller address is not found in the Cartridge API.
async fn fetch_controller_constructor_calldata(
    cartridge_api_url: &Url,
    address: Felt,
) -> anyhow::Result<Option<Vec<Felt>>> {
    let account_data_url = cartridge_api_url.join("/accounts/calldata")?;

    let body = serde_json::json!({
        "address": format!("{:#066x}", address)
    });

    let client = reqwest::Client::new();
    let response = client
        .post(account_data_url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    let response = response.text().await?;
    if response.contains("Address not found") {
        Ok(None)
    } else {
        let account = serde_json::from_str::<CartridgeAccountResponse>(&response)?;
        Ok(Some(account.calldata))
    }
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
pub async fn handle_cartridge_estimate_fee(
    paymaster_address: ContractAddress,
    paymaster_private_key: Felt,
    tx: &ExecutableTxWithHash,
    chain_id: ChainId,
    state: Arc<Box<dyn StateProvider>>,
    cartridge_api_url: &Url,
) -> anyhow::Result<Option<ExecutableTxWithHash>> {
    let paymaster_nonce = state.nonce(paymaster_address)?;

    if let ExecutableTx::Invoke(InvokeTx::V3(v3)) = &tx.transaction {
        let maybe_controller_address = v3.sender_address;

        // Avoid deploying the controller account if it is already deployed.
        if state.class_hash_of_contract(maybe_controller_address)?.is_some() {
            return Ok(None);
        }

        debug!(contract_address = ?maybe_controller_address, "Deploying controller account.");
        if let tx @ Some(..) = craft_deploy_cartridge_controller_tx(
            cartridge_api_url,
            maybe_controller_address,
            paymaster_address,
            paymaster_private_key,
            chain_id,
            paymaster_nonce,
        )
        .await?
        {
            return Ok(tx);
        }
    }

    Ok(None)
}

/// Crafts a deploy controller transaction for a cartridge controller.
///
/// Returns None if the provided `controller_address` is not registered in the Cartridge API.
pub async fn craft_deploy_cartridge_controller_tx(
    cartridge_api_url: &Url,
    controller_address: ContractAddress,
    paymaster_address: ContractAddress,
    paymaster_private_key: Felt,
    chain_id: ChainId,
    paymaster_nonce: Option<Felt>,
) -> anyhow::Result<Option<ExecutableTxWithHash>> {
    if let Some(calldata) =
        fetch_controller_constructor_calldata(cartridge_api_url, controller_address.into()).await?
    {
        let call = Call {
            to: DEFAULT_UDC_ADDRESS.into(),
            selector: selector!("deployContract"),
            calldata,
        };

        let mut tx = InvokeTxV3 {
            chain_id,
            tip: 0_u64,
            signature: vec![],
            paymaster_data: vec![],
            account_deployment_data: vec![],
            sender_address: paymaster_address,
            calldata: encode_calls(vec![call]),
            nonce: paymaster_nonce.unwrap_or(Felt::ZERO),
            resource_bounds: ResourceBoundsMapping::default(),
            nonce_data_availability_mode: katana_primitives::da::DataAvailabilityMode::L1,
            fee_data_availability_mode: katana_primitives::da::DataAvailabilityMode::L1,
        };

        let tx_hash = InvokeTx::V3(tx.clone()).calculate_hash(false);

        let signer = LocalWallet::from(SigningKey::from_secret_scalar(paymaster_private_key));
        let signature = futures::executor::block_on(signer.sign_hash(&tx_hash))
            .expect("failed to sign hash with paymaster");
        tx.signature = vec![signature.r, signature.s];

        let tx = ExecutableTxWithHash::new(ExecutableTx::Invoke(InvokeTx::V3(tx)));

        Ok(Some(tx))
    } else {
        Ok(None)
    }
}
