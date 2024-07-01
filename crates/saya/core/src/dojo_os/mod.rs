//! Starknet OS types.
// SNOS is based on blockifier, which is not in sync with
// current primitives.
// And SNOS is for now not used. This work must be resume once
// SNOS is actualized.
// mod felt;
// pub mod input;
// pub mod transaction;

use std::time::Duration;

use anyhow::{bail, Context};
use dojo_world::migration::TxnConfig;
use dojo_world::utils::TransactionExt;
use itertools::chain;
use starknet::accounts::{Account, Call, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{
    BlockId, BlockTag, Felt, TransactionExecutionStatus, TransactionStatus,
};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet::signers::{LocalWallet, SigningKey};
use tokio::time::sleep;
use url::Url;
// will need to be read from the environment for chains other than sepoia
pub const STARKNET_URL: &str = "https://free-rpc.nethermind.io/sepolia-juno/v0_7";
pub const CHAIN_ID: &str = "0x00000000000000000000000000000000000000000000534e5f5345504f4c4941";
pub const SIGNER_ADDRESS: &str =
    "0x00ceE714eAF27390e630c62aa4b51319f9EdA813d6DDd12dA0ae8Ce00453cb4b";
pub const SIGNER_KEY: &str = "0x01c49f9a0f5d2ca87fe7bb0530c611f91faf4adda6b7fcff479ce92ea13b1b4c";

lazy_static::lazy_static!(
    pub static ref STARKNET_ACCOUNT: SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> = {
        let provider = JsonRpcClient::new(HttpTransport::new(
            Url::parse(STARKNET_URL).unwrap(),
        ));

        let signer = Felt::from_hex(SIGNER_KEY).expect("invalid signer hex");
        let signer = LocalWallet::from(SigningKey::from_secret_scalar(signer));

        let address = Felt::from_hex(SIGNER_ADDRESS).expect("invalid signer address");
        let chain_id = Felt::from_hex(CHAIN_ID).expect("invalid chain id");

        let mut account = SingleOwnerAccount::new(provider, signer, address, chain_id, ExecutionEncoding::New);
        account.set_block_id(BlockId::Tag(BlockTag::Pending));
        account
    };
);

pub async fn starknet_apply_diffs(
    world: Felt,
    new_state: Vec<Felt>,
    program_output: Vec<Felt>,
    program_hash: Felt,
    nonce: Felt,
) -> anyhow::Result<String> {
    let calldata = chain![
        vec![Felt::from(new_state.len() as u64 / 2)].into_iter(),
        new_state.clone().into_iter(),
        program_output.into_iter(),
        vec![program_hash],
    ]
    .collect();

    let txn_config = TxnConfig { wait: true, receipt: true, ..Default::default() };
    let tx = STARKNET_ACCOUNT
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

        let status =
            match STARKNET_ACCOUNT.provider().get_transaction_status(tx.transaction_hash).await {
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
