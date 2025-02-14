use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use alloy::primitives::{Uint, U256};
use alloy::providers::ProviderBuilder;
use alloy::sol;
use anyhow::Result;
use cainome::cairo_serde::EthAddress;
use cainome::rs::abigen;
use dojo_test_utils::sequencer::{get_default_test_config, TestSequencer};
use dojo_utils::TransactionWaiter;
use katana_messaging::MessagingConfig;
use katana_node::config::sequencing::SequencingConfig;
use katana_primitives::felt;
use katana_primitives::utils::transaction::{
    compute_l1_handler_tx_hash, compute_l1_to_l2_message_hash, compute_l2_to_l1_message_hash,
};
use katana_rpc_types::receipt::ReceiptBlock;
use rand::Rng;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::contract::ContractFactory;
use starknet::core::types::{
    BlockId, BlockTag, ContractClass, Felt, Hash256, MsgFromL1, Transaction,
    TransactionFinalityStatus, TransactionReceipt,
};
use starknet::core::utils::get_contract_address;
use starknet::macros::selector;
use starknet::providers::Provider;

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

    let messaging_config = MessagingConfig {
        chain: "ethereum".to_string(),
        rpc_url: format!("http://localhost:{}", port),
        contract_address: core_contract.address().to_string(),
        interval: 2,
        from_block: 0,
    };

    let mut config = get_default_test_config(SequencingConfig::default());
    config.messaging = Some(messaging_config);
    let sequencer = TestSequencer::start(config).await;

    let katana_account = sequencer.account();

    // Deploy test L2 contract that can send/receive messages to/from L1
    let l2_test_contract = {
        // Prepare contract declaration params
        let path = PathBuf::from("tests/test_data/cairo_l1_msg_contract.json");
        let (contract, compiled_hash) = common::prepare_contract_declaration_params(&path).unwrap();

        // Declare the contract
        let class_hash = contract.class_hash();
        let res = katana_account.declare_v2(contract.into(), compiled_hash).send().await.unwrap();

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
        let address = get_contract_address(Felt::ZERO, class_hash, &[], Felt::ZERO);

        // Deploy the contract using UDC
        let res = ContractFactory::new(class_hash, &katana_account)
            .deploy_v1(Vec::new(), Felt::ZERO, false)
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
        let mut l1_tx_calldata = vec![Felt::from_bytes_be_slice(sender.as_slice())];
        l1_tx_calldata.extend(calldata.iter().map(|x| Felt::from(*x)));

        // Compute transaction hash
        let tx_hash = compute_l1_handler_tx_hash(
            Felt::ZERO,
            recipient,
            selector,
            &l1_tx_calldata,
            sequencer.provider().chain_id().await.unwrap(),
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
        let receipt_res = katana_account
            .provider()
            .get_transaction_receipt(tx.transaction_hash)
            .await
            .expect("failed to get receipt");

        match receipt_res.block {
            ReceiptBlock::Block { .. } => {
                let TransactionReceipt::L1Handler(receipt) = receipt_res.receipt else {
                    panic!("invalid receipt type");
                };

                let msg_hash = compute_l1_to_l2_message_hash(
                    sender.as_slice().try_into().unwrap(),
                    recipient,
                    selector,
                    &calldata.iter().map(|x| Felt::from(*x)).collect::<Vec<_>>(),
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

    // Send message from L2 to L1 testing must be done using Saya or part of
    // it to ensure the settlement contract is test on piltover and its `update_state` method.
}

#[tokio::test]
async fn estimate_message_fee() -> Result<()> {
    let config = get_default_test_config(SequencingConfig::default());
    let sequencer = TestSequencer::start(config).await;

    let provider = sequencer.provider();
    let account = sequencer.account();

    // Declare and deploy a l1 handler contract
    let path = PathBuf::from("tests/test_data/cairo_l1_msg_contract.json");
    let (contract, compiled_hash) = common::prepare_contract_declaration_params(&path)?;
    let class_hash = contract.class_hash();

    let res = account.declare_v2(contract.into(), compiled_hash).send().await?;
    TransactionWaiter::new(res.transaction_hash, account.provider()).await?;

    // Deploy the contract using UDC
    let res = ContractFactory::new(class_hash, &account)
        .deploy_v1(Vec::new(), Felt::ZERO, false)
        .send()
        .await?;

    TransactionWaiter::new(res.transaction_hash, account.provider()).await?;

    // Compute the contract address of the l1 handler contract
    let l1handler_address = get_contract_address(Felt::ZERO, class_hash, &[], Felt::ZERO);

    // This is the function signature of the #[l1handler] function we''re gonna call. Though the
    // function accepts two arguments, we're only gonna pass one argument, as the `from_address`
    // of the `MsgFromL1` will be automatically injected as part of the function calldata.
    //
    // See https://docs.starknet.io/documentation/architecture_and_concepts/Network_Architecture/messaging-mechanism/#l1-l2-messages.
    //
    // #[l1_handler]
    // fn msg_handler_value(ref self: ContractState, from_address: felt252, value: felt252)

    let entry_point_selector = selector!("msg_handler_value");
    let payload = vec![felt!("123")];
    let from_address = felt!("0x1337");
    let to_address = l1handler_address;

    let msg = MsgFromL1 {
        payload,
        to_address,
        entry_point_selector,
        from_address: from_address.try_into()?,
    };

    let result = provider.estimate_message_fee(msg, BlockId::Tag(BlockTag::Pending)).await;
    assert!(result.is_ok());

    // #[derive(Drop, Serde)]
    // struct MyData {
    //     a: felt252,
    //     b: felt252,
    // }
    //
    // #[l1_handler]
    // fn msg_handler_struct(ref self: ContractState, from_address: felt252, data: MyData)

    let entry_point_selector = selector!("msg_handler_struct");
    // [ MyData.a , MyData.b ]
    let payload = vec![felt!("1"), felt!("2")];
    let from_address = felt!("0x1337");
    let to_address = l1handler_address;

    let msg = MsgFromL1 {
        payload,
        to_address,
        entry_point_selector,
        from_address: from_address.try_into()?,
    };

    let result = provider.estimate_message_fee(msg, BlockId::Tag(BlockTag::Pending)).await;
    assert!(result.is_ok());

    Ok(())
}
