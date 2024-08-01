use anyhow::Context;
use dojo_utils::{TransactionExt, TxnConfig};
use itertools::Itertools;
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::types::{
    Felt, InvokeTransactionResult, TransactionExecutionStatus, TransactionStatus,
};
use starknet::core::utils::get_selector_from_name;
use starknet_crypto::poseidon_hash_many;
use tracing::trace;

use crate::SayaStarknetAccount;

use super::utils::wait_for_sent_transaction;

pub async fn starknet_verify(
    fact_registry_address: Felt,
    serialized_proof: Vec<Felt>,
    cairo_version: Felt,
    account: &SayaStarknetAccount,
) -> anyhow::Result<(String, Felt)> {
    if serialized_proof.len() > 2000 {
        trace!(
            "Calldata too long at: {} felts, transaction could fail, splitting it.",
            serialized_proof.len()
        );
    }

    let txn_config = TxnConfig { wait: true, receipt: true, ..Default::default() };

    let mut nonce = account.get_nonce().await?;
    let mut hashes = Vec::new();

    for fragment in serialized_proof.into_iter().chunks(2000).into_iter() {
        let mut fragment = fragment.collect::<Vec<_>>();
        let hash = poseidon_hash_many(&fragment);
        hashes.push(hash);

        fragment.insert(0, fragment.len().into());

        let tx = account
            .execute_v1(vec![Call {
                to: fact_registry_address,
                selector: get_selector_from_name("publish_fragment").expect("invalid selector"),
                calldata: fragment,
            }])
            .nonce(nonce)
            // .max_fee(576834050002014927u64.into())
            .send_with_cfg(&txn_config)
            .await
            .context("Failed to send `publish_fragment` transaction.")?;

        trace!("Sent `publish_fragment` transaction {:#x}", tx.transaction_hash);

        wait_for_sent_transaction(tx, account).await?;

        nonce += &1u64.into();
    }

    let calldata = [Felt::from(hashes.len() as u64)]
        .into_iter()
        .chain(hashes.into_iter())
        .chain([cairo_version].into_iter())
        .collect::<Vec<_>>();

    let nonce = account.get_nonce().await?;
    let tx = account
        .execute_v1(vec![Call {
            to: fact_registry_address,
            selector: get_selector_from_name("verify_and_register_fact_from_fragments")
                .expect("invalid selector"),
            calldata,
        }])
        .nonce(nonce)
        .send_with_cfg(&txn_config)
        .await
        .context("Failed to send `verify_and_register_fact_from_fragments` transaction.")?;

    let transaction_hash = format!("{:#x}", tx.transaction_hash);
    wait_for_sent_transaction(tx, account).await?;

    Ok((transaction_hash, nonce + &1u64.into()))
}

async fn wait_for(
    tx: InvokeTransactionResult,
    starknet_config: StarknetAccountData,
) -> anyhow::Result<()> {
    let start_fetching = std::time::Instant::now();
    let wait_for = Duration::from_secs(60);
    let execution_status = loop {
        if start_fetching.elapsed() > wait_for {
            anyhow::bail!("Transaction not mined in {} seconds.", wait_for.as_secs());
        }

        let account = get_starknet_account(starknet_config.clone())?;
        let account = account.lock().await;

        let status = match account.provider().get_transaction_status(tx.transaction_hash).await {
            Ok(status) => status,
            Err(_e) => {
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        break match status {
            TransactionStatus::Received => {
                info!("Transaction received.");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
            TransactionStatus::Rejected => {
                anyhow::bail!("Transaction {:#x} rejected.", tx.transaction_hash);
            }
            TransactionStatus::AcceptedOnL2(execution_status) => execution_status,
            TransactionStatus::AcceptedOnL1(execution_status) => execution_status,
        };
    };

    match execution_status {
        TransactionExecutionStatus::Succeeded => {
            info!("Transaction accepted on L2.");
        }
        TransactionExecutionStatus::Reverted => {
            anyhow::bail!("Transaction failed with.");
        }
    }

    Ok(())
}
