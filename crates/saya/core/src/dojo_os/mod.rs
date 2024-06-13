//! Starknet OS types.
// SNOS is based on blockifier, which is not in sync with
// current primitives.
// And SNOS is for now not used. This work must be resume once
// SNOS is actualized.
// mod felt;
// pub mod input;
// pub mod transaction;

use once_cell::sync::OnceCell;
use std::time::Duration;

use anyhow::{bail, Context};
use dojo_world::migration::TxnConfig;
use dojo_world::utils::TransactionExt;
use itertools::chain;
use starknet::accounts::{Account, Call, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{FieldElement, TransactionExecutionStatus, TransactionStatus};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet::signers::{LocalWallet, SigningKey};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::data_availability::StarknetAccountInput;
pub static STARKNET_ACCOUNT: OnceCell<
    Arc<Mutex<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>>>,
> = OnceCell::new();

pub fn get_starknet_account(
    config: StarknetAccountInput,
) -> anyhow::Result<Arc<Mutex<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>>>> {
    Ok(STARKNET_ACCOUNT
        .get_or_init(|| {
            let provider = JsonRpcClient::new(HttpTransport::new(config.starknet_url));
            let signer = FieldElement::from_hex_be(&config.signer_key).expect("invalid signer hex");
            let signer = LocalWallet::from(SigningKey::from_secret_scalar(signer));

            let address =
                FieldElement::from_hex_be(&config.signer_address).expect("invalid signer address");
            let chain_id = FieldElement::from_hex_be(&config.chain_id).expect("invalid chain id");

            let account = SingleOwnerAccount::new(
                provider,
                signer,
                address,
                chain_id,
                ExecutionEncoding::New,
            );
            Arc::new(Mutex::new(account))
        })
        .clone())
}

pub async fn starknet_apply_diffs(
    world: FieldElement,
    new_state: Vec<FieldElement>,
    program_output: Vec<FieldElement>,
    program_hash: FieldElement,
    nonce: FieldElement,
    starknet_account: StarknetAccountInput,
) -> anyhow::Result<String> {
    let calldata = chain![
        vec![FieldElement::from(new_state.len() as u64 / 2)].into_iter(),
        new_state.clone().into_iter(),
        program_output.into_iter(),
        vec![program_hash],
    ]
    .collect::<Vec<FieldElement>>();
    let account = get_starknet_account(starknet_account)?;
    let account = account.lock().await;
    let txn_config = TxnConfig { wait: true, receipt: true, ..Default::default() };
    let tx = account
        .execute(vec![Call {
            to: world,
            selector: get_selector_from_name("upgrade_state").expect("invalid selector"),
            calldata,
        }])
        .nonce(nonce)
        .send_with_cfg(&txn_config)
        .await
        .context("Failed to send `upgrade state` transaction.")?;

    let start_fetching = std::time::Instant::now();
    let wait_for = Duration::from_secs(60);
    let execution_status = loop {
        if start_fetching.elapsed() > wait_for {
            bail!("Transaction not mined in {} seconds.", wait_for.as_secs());
        }

        let status = match account.provider().get_transaction_status(tx.transaction_hash).await {
            Ok(status) => status,
            Err(_e) => {
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        break match status {
            TransactionStatus::Received => {
                println!("Transaction received.");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
            TransactionStatus::Rejected => {
                bail!("Transaction {:#x} rejected.", tx.transaction_hash);
            }
            TransactionStatus::AcceptedOnL2(execution_status) => execution_status,
            TransactionStatus::AcceptedOnL1(execution_status) => execution_status,
        };
    };

    match execution_status {
        TransactionExecutionStatus::Succeeded => {
            println!("Transaction accepted on L2.");
        }
        TransactionExecutionStatus::Reverted => {
            bail!("Transaction failed with.");
        }
    }

    Ok(format!("{:#x}", tx.transaction_hash))
}
