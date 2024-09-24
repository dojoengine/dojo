use std::time::Duration;
use std::vec;

use dojo_utils::{TransactionExt, TxnConfig};
use itertools::Itertools;
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::types::Felt;
use starknet::core::utils::get_selector_from_name;
use starknet_crypto::poseidon_hash_many;
use tokio::time::sleep;
use tracing::{info, trace};

use super::utils::wait_for_sent_transaction;
use crate::error::Error;
use crate::{retry, SayaStarknetAccount, LOG_TARGET};

const CHUNK_SIZE: usize = 800;
pub async fn starknet_verify(
    fact_registry_address: Felt,
    serialized_proof: Vec<Felt>,
    cairo_version: Felt,
    account: &SayaStarknetAccount,
) -> Result<(String, Felt),Error> {
    if serialized_proof.len() > CHUNK_SIZE {
        trace!(target: LOG_TARGET,
            "Calldata too long at: {} felts, transaction could fail, splitting it.",
            serialized_proof.len()
        );
    }

    let txn_config = TxnConfig { wait: true, receipt: true, ..Default::default() };

    sleep(Duration::from_secs(2)).await;
    let mut nonce = account.get_nonce().await?;
    let mut hashes = Vec::new();

    for fragment in serialized_proof.into_iter().chunks(CHUNK_SIZE).into_iter() {
        let mut fragment = fragment.collect::<Vec<_>>();
        let hash = poseidon_hash_many(&fragment);
        hashes.push(hash);
        fragment.insert(0, fragment.len().into());

        let tx = retry!(account
            .execute_v1(vec![Call {
                to: fact_registry_address,
                selector: get_selector_from_name("publish_fragment").expect("invalid selector"),
                calldata: fragment.clone(),
            }])
            .nonce(nonce)
            .send_with_cfg(&txn_config)
            ).unwrap(); //TODO: handle this

        wait_for_sent_transaction(tx.clone(), account).await?;

        trace!(target: LOG_TARGET, "Sent `publish_fragment` transaction {:#x}", tx.transaction_hash);
    }

    info!(target: LOG_TARGET, "Sent all proof fragments.");

    let calldata = [Felt::from(hashes.len() as u64)]
        .into_iter()
        .chain(hashes.into_iter())
        .chain([cairo_version].into_iter())
        .collect::<Vec<_>>();

    sleep(Duration::from_secs(2)).await;
    nonce = account.get_nonce().await?; 

    let tx = retry!(account
        .execute_v1(vec![Call {
            to: fact_registry_address,
            selector: get_selector_from_name("verify_and_register_fact_from_fragments")
                .expect("invalid selector"),
            calldata: calldata.clone(),
        }])
        .nonce(nonce)
        .send_with_cfg(&txn_config)
        ).unwrap();
        
    let transaction_hash = format!("{:#x}", tx.transaction_hash);
    wait_for_sent_transaction(tx, account).await?;

    Ok((transaction_hash, nonce + Felt::ONE))
}
