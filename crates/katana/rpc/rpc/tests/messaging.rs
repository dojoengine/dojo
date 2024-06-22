use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use alloy::primitives::{Uint, U256};
use alloy::providers::{ProviderBuilder, WalletProvider};
use alloy::sol;
use cainome::cairo_serde::EthAddress;
use cainome::rs::abigen;
use dojo_world::utils::TransactionWaiter;
use katana_primitives::utils::transaction::{
    compute_l1_handler_tx_hash, compute_l1_to_l2_message_hash, compute_l2_to_l1_message_hash,
};
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use rand::Rng;
use serde_json::json;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::contract::ContractFactory;
use starknet::core::types::{
    BlockId, BlockTag, ContractClass, FieldElement, Hash256, MaybePendingTransactionReceipt,
    Transaction, TransactionFinalityStatus, TransactionReceipt,
};
use starknet::core::utils::get_contract_address;
use starknet::macros::selector;
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
    // TODO: If there's a way to get the endpoint of anvil from the `l1_provider`, we could
    // remove that and use default anvil to let the OS assign the port.
    let port: u16 = rand::thread_rng().gen_range(35000..65000);

    let l1_provider = {
        ProviderBuilder::new()
            .with_recommended_fillers()
            .on_anvil_with_wallet_and_config(|anvil| anvil.port(port))
    };

    // Deploy the core messaging contract on L1
    let core_contract = StarknetContract::deploy(&l1_provider).await.unwrap();

    // Deploy test contract on L1 used to send/receive messages to/from L2
    let l1_test_contract = Contract1::deploy(&l1_provider, *core_contract.address()).await.unwrap();

    // Prepare Katana + Messaging Contract
    let messaging_config = json!({
        "chain": "ethereum",
        "rpc_url": format!("http://localhost:{}", port),
        "contract_address": core_contract.address().to_string(),
        "sender_address": l1_provider.default_signer_address(),
        "private_key": "",
        "interval": 2,
        "from_block": 0
    })
    .to_string();

    let dir = tempdir().expect("failed creating temp dir");
    let path = dir.path().join("temp-anvil-messaging.json");
    std::fs::write(&path, messaging_config.as_bytes()).expect("failed to write config to file");

    let katana_runner = KatanaRunner::new_with_config(KatanaRunnerConfig {
        n_accounts: 2,
        messaging: Some(path.to_str().unwrap().to_string()),
        ..Default::default()
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
        // The L1 sender address
        let sender = l1_test_contract.address();
        // The L2 contract address to send the message to
        let recipient = l2_test_contract;
        // The L2 contract function to call
        let selector = selector!("msg_handler_value");
        // The L2 contract function arguments
        let calldata = [123u8];
        // Get the current L1 -> L2 message nonce
        let nonce = core_contract.l1ToL2MessageNonce().call().await.expect("get nonce")._0;

        // Send message to L2
        let call = l1_test_contract
            .sendMessage(
                U256::from_str(&recipient.to_string()).unwrap(),
                U256::from_str(&selector.to_string()).unwrap(),
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

        // Wait for the tx to be mined on L2 (Katana)
        tokio::time::sleep(Duration::from_secs(5)).await;

        // In an l1_handler transaction, the first element of the calldata is always the Ethereum
        // address of the sender (msg.sender).
        let mut l1_tx_calldata = vec![FieldElement::from_byte_slice_be(sender.as_slice()).unwrap()];
        l1_tx_calldata.extend(calldata.iter().map(|x| FieldElement::from(*x)));

        // Compute transaction hash
        let tx_hash = compute_l1_handler_tx_hash(
            FieldElement::ZERO,
            recipient,
            selector,
            &l1_tx_calldata,
            katana_runner.provider().chain_id().await.unwrap(),
            nonce.to::<u64>().into(),
        );

        // fetch the transaction
        let tx = katana_account
            .provider()
            .get_transaction_by_hash(tx_hash)
            .await
            .expect("failed to get l1 handler tx");

        let Transaction::L1Handler(ref tx) = tx else {
            panic!("invalid transaction type");
        };

        // Assert the transaction fields
        assert_eq!(tx.contract_address, recipient);
        assert_eq!(tx.entry_point_selector, selector);
        assert_eq!(tx.calldata, l1_tx_calldata);

        // fetch the receipt
        let receipt = katana_account
            .provider()
            .get_transaction_receipt(tx.transaction_hash)
            .await
            .expect("failed to get receipt");

        match receipt {
            MaybePendingTransactionReceipt::Receipt(receipt) => {
                let TransactionReceipt::L1Handler(receipt) = receipt else {
                    panic!("invalid receipt type");
                };

                let msg_hash = compute_l1_to_l2_message_hash(
                    sender.as_slice().try_into().unwrap(),
                    recipient,
                    selector,
                    &calldata.iter().map(|x| FieldElement::from(*x)).collect::<Vec<_>>(),
                    nonce.to::<u64>(),
                );

                let msg_fee = core_contract
                    .l1ToL2Messages(msg_hash)
                    .call()
                    .await
                    .expect("failed to get msg fee");

                assert_ne!(msg_fee._0, U256::ZERO, "msg fee must be non-zero if exist");
                assert_eq!(receipt.message_hash, Hash256::from_bytes(msg_hash.0));
            }

            _ => {
                panic!("Error, No Receipt TransactionReceipt")
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

        // Wait for the tx to be mined on L1 (Anvil)
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Query the core messaging contract to check that the l2 -> l1 message hash have been
        // registered. If the message is registered, calling `l2ToL1Messages` of the L1 core
        // contract with the message hash should return a non-zero value.

        let l2_l1_msg_hash = compute_l2_to_l1_message_hash(
            l2_test_contract,
            l1_contract_address,
            &[FieldElement::TWO],
        );

        let msg_fee = core_contract
            .l2ToL1Messages(l2_l1_msg_hash)
            .call()
            .await
            .expect("failed to get msg fee");

        assert_ne!(msg_fee._0, U256::ZERO, "msg fee must be non-zero if exist");

        // We then consume the message.
        // Upon consuming the message, the value returned by `l2ToL1Messages` should be zeroed.

        // The L2 contract address that sent the message
        let from_address = U256::from_str(&l2_test_contract.to_string()).unwrap();
        // The message payload
        let payload = vec![U256::from(2)];

        let receipt = l1_test_contract
            .consumeMessage(from_address, payload)
            .gas(12000000)
            .nonce(4)
            .send()
            .await
            .expect("failed to send tx")
            .get_receipt()
            .await
            .expect("error getting transaction receipt");

        assert!(receipt.status(), "failed to consume L2 message from L1");

        // Check that the message fee is zero after consuming the message.
        let msg_fee = core_contract
            .l2ToL1Messages(l2_l1_msg_hash)
            .call()
            .await
            .expect("failed to get msg fee");

        assert_eq!(msg_fee._0, U256::ZERO, "msg fee must be zero after consuming");
    }
}
