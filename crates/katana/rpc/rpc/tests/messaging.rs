use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use alloy::primitives::{Uint, U256};
use alloy::sol;
use cainome::cairo_serde::EthAddress;
use cainome::rs::abigen;
use dojo_world::utils::TransactionWaiter;
use katana_runner::{AnvilRunner, KatanaRunner, KatanaRunnerConfig};
use serde_json::json;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::contract::ContractFactory;
use starknet::core::types::{
    BlockId, BlockTag, ContractClass, FieldElement, Transaction, TransactionFinalityStatus,
};
use starknet::core::utils::get_contract_address;
use starknet::macros::felt;
use starknet::providers::Provider;
use tempfile::tempdir;

mod common;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    StarknetContract,
    "tests/test_data/solidity/StarknetMessagingLocalCompiled.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    Contract1,
    "tests/test_data/solidity/Contract1Compiled.json"
);

abigen!(CairoMessagingContract, "crates/katana/rpc/rpc/tests/test_data/cairo_l1_msg_contract.json");

#[tokio::test(flavor = "multi_thread")]
async fn test_messaging() {
    // Prepare Anvil + Messaging Contracts
    let anvil_runner = AnvilRunner::new().await.unwrap();
    let l1_provider = anvil_runner.provider();

    // Deploy the core messaging contract on L1
    let core_contract = StarknetContract::deploy(anvil_runner.provider()).await.unwrap();
    // Deploy test contract on L1 used to send/receive messages to/from L2
    let l1_test_contract = Contract1::deploy(l1_provider, *core_contract.address()).await.unwrap();

    // Prepare Katana + Messaging Contract
    let messaging_config = json!({
        "chain": "ethereum",
        "rpc_url": anvil_runner.endpoint,
        "contract_address": core_contract.address().to_string(),
        "sender_address": anvil_runner.address(),
        "private_key": anvil_runner.secret_key(),
        "interval": 2,
        "from_block": 0
    })
    .to_string();

    let dir = tempdir().expect("failed creating temp dir");
    let path = dir.path().join("temp-anvil-messaging.json");
    std::fs::write(&path, messaging_config.as_bytes()).expect("failed to write config to file");

    let katana_runner = KatanaRunner::new_with_config(KatanaRunnerConfig {
        n_accounts: 2,
        disable_fee: false,
        block_time: None,
        port: None,
        program_name: None,
        run_name: None,
        messaging: Some(path.to_str().unwrap().to_string()),
    })
    .unwrap();

    let katana_account = katana_runner.account(0);

    // Deploy test L2 contract that can send/receive messages to/from L1
    let l2_test_contract = {
        // Prepare contract declaration params
        let path = PathBuf::from("tests/test_data/cairo_l1_msg_contract.json");
        let (contract, compiled_hash) = common::prepare_contract_declaration_params(&path).unwrap();

        // Declare the contract
        let class_hash = contract.class_hash();
        let res = katana_account.declare(contract.into(), compiled_hash).send().await.unwrap();

        // The waiter already checks that the transaction is accepted and succeeded on L2.
        TransactionWaiter::new(res.transaction_hash, katana_account.provider())
            .await
            .expect("declare tx failed");

        // Checks that the class was indeed declared
        let block_id = BlockId::Tag(BlockTag::Latest);
        let actual_class = katana_account.provider().get_class(block_id, class_hash).await.unwrap();

        let ContractClass::Sierra(class) = actual_class else { panic!("Invalid class type") };
        assert_eq!(class.class_hash(), class_hash, "invalid declared class"); // just to make sure the rpc returns the correct class

        // Compute the contract address
        let address = get_contract_address(FieldElement::ZERO, class_hash, &[], FieldElement::ZERO);

        // Deploy the contract using UDC
        let res = ContractFactory::new(class_hash, &katana_account)
            .deploy(Vec::new(), FieldElement::ZERO, false)
            .send()
            .await
            .expect("Unable to deploy contract");

        // The waiter already checks that the transaction is accepted and succeeded on L2.
        TransactionWaiter::new(res.transaction_hash, katana_account.provider())
            .await
            .expect("deploy tx failed");

        // Checks that the class was indeed deployed with the correct class
        let actual_class_hash = katana_account
            .provider()
            .get_class_hash_at(block_id, address)
            .await
            .expect("failed to get class hash at address");

        assert_eq!(actual_class_hash, class_hash, "invalid deployed class");

        address
    };

    // Send message from L1 to L2
    {
        // The L2 contract address to send the message to
        let recipient = l2_test_contract;
        // The L2 contract function to call
        // TODO: compute the function hash from the function signature instead of hardcoding it
        let function = "0x005421de947699472df434466845d68528f221a52fce7ad2934c5dae2e1f1cdc";
        // The L2 contract function arguments
        let calldata = vec![123];

        let call = l1_test_contract
            .sendMessage(
                U256::from_str(&recipient.to_string()).unwrap(),
                U256::from_str(function).unwrap(),
                calldata.iter().map(|x| U256::from(*x)).collect::<Vec<_>>(),
            )
            .gas(12000000)
            .value(Uint::from(1));

        let receipt = call
            .send()
            .await
            .expect("failed to send tx")
            .get_receipt()
            .await
            .expect("error getting transaction receipt");

        assert!(receipt.status(), "failed to send L1 -> L2 message");

        // TODO: query the core messaging contract to check that the message hash do exist

        // Wait for the tx to be mined on L2 (Katana)
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Check that the transaction was indeed received by Katana
        let block_id = BlockId::Tag(BlockTag::Latest);
        let tx = katana_account
            .provider()
            .get_transaction_by_block_id_and_index(block_id, 0)
            .await
            .unwrap();

        match tx {
            Transaction::L1Handler(ref l1_handler_transaction) => {
                let calldata = &l1_handler_transaction.calldata;

                // TODO: compute the tx hash using the message params, we're not really asserting
                // the hash to be correct as we just hardcoded it.
                //
                // hint: use `compute_l1_handler_tx_hash` and assert with the actual hash instead of
                // the hardcoded value
                assert_eq!(
                    tx.transaction_hash(),
                    &felt!("0x00c33cc113afc56bc878034908472770cb13eda6ad8ad91feb25fd4e5c9196a0")
                );

                // TODO: compute the l1 message hash and assert with the actual message hash in the
                // l1 handler receipt hint: use `compute_l1_message_hash`

                assert_eq!(FieldElement::to_string(&calldata[1]), "123")
            }
            _ => {
                panic!("Error, No L1handler transaction")
            }
        }
    }

    // Send message from L2 to L1
    {
        // The L1 contract address to send the message to
        let l1_contract_address = l1_test_contract.address();
        let l1_contract_address = FieldElement::from_str(&l1_contract_address.to_string()).unwrap();

        let l2_contract = CairoMessagingContract::new(l2_test_contract, &katana_account);

        // Send message to L1
        let res = l2_contract
            .send_message_value(&EthAddress::from(l1_contract_address), &FieldElement::TWO)
            .send()
            .await
            .expect("Call to send_message_value failed");

        TransactionWaiter::new(res.transaction_hash, katana_account.provider())
            .with_tx_status(TransactionFinalityStatus::AcceptedOnL2)
            .await
            .expect("send message to l1 tx failed");

        // The L2 contract address that sent the message
        let from_address = U256::from_str(&l2_test_contract.to_string()).unwrap();
        // The message payload
        let payload = vec![U256::from(2)];

        let call = l1_test_contract
            .consumeMessage(from_address, payload)
            .value(Uint::from(1))
            .gas(12000000)
            .nonce(5);

        // Wait for the tx to be mined on L1 (Anvil)
        tokio::time::sleep(Duration::from_secs(8)).await;

        let receipt = call
            .send()
            .await
            .expect("failed to send tx")
            .get_receipt()
            .await
            .expect("error getting transaction receipt");

        assert!(receipt.status(), "failed to consume L2 message from L1");

        // TODO: query the core messaging contract to check that the message hash do exist
    }
}
