//! Integration tests for starknet commands using Sepolia testnet.
//!
//! These tests use real data from block 5,000,000 on Sepolia, which is already
//! settled on L1 and its state will not change.

use url::Url;

use super::block::BlockArgs;
use super::state::{BalanceArgs, ClassAtArgs, ClassByHashArgs, ClassHashAtArgs, NonceArgs};
use super::transaction::{ReceiptArgs, StatusArgs, TransactionArgs};
use super::{BlockIdOption, OutputOptions};
use crate::commands::options::starknet::StarknetOptions;

/// Sepolia RPC URL for testing
const SEPOLIA_RPC_URL: &str = "https://api.cartridge.gg/x/starknet/sepolia";

/// Block number used for all tests (settled on L1)
const TEST_BLOCK_NUMBER: u64 = 5_000_000;

/// Block hash for block 5,000,000
const TEST_BLOCK_HASH: &str =
    "0x47569582123bf39767fe0cd204d244d5c014b7dd88804852c48fecf315e96f1";

/// First transaction hash in block 5,000,000
const TEST_TX_HASH: &str = "0x7280a6f96f587021e1fe396dfd899527c193ac58c63e4c9976a989d5520ec3d";

/// Second transaction hash in block 5,000,000
const TEST_TX_HASH_2: &str = "0x5afbd114cbde3ad96f0774124068933032aab54c8e10bd4cfa212a5643c44e9";

/// Sender address of the first transaction
const TEST_ADDRESS: &str = "0x4f4e29add19afa12c868ba1f4439099f225403ff9a71fe667eebb50e13518d3";

/// Another active account for batch testing
const TEST_ADDRESS_2: &str = "0x7dcce8b0d3d1d8f9636e78870f30a1ece8028792598a4b06b9ac5c1fd75ce43";

/// Class hash at the test address
const TEST_CLASS_HASH: &str = "0x4d9d2b2e26f94fad32e7b7a7e710286636322d5905f1cd64dc58a144294e6";

/// STRK token contract address
const STRK_TOKEN_ADDRESS: &str =
    "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d";

/// STRK token class hash
const STRK_CLASS_HASH: &str =
    "0x4ad3c1dc8413453db314497945b6903e1c766495a1e60492d44da9c2a986e4b";

fn create_starknet_options() -> StarknetOptions {
    StarknetOptions {
        rpc_url: Some(Url::parse(SEPOLIA_RPC_URL).unwrap()),
        use_blake2s_casm_class_hash: false,
        rpc_headers: vec![],
    }
}

fn create_output_options() -> OutputOptions {
    OutputOptions { raw: true, dec: false }
}

fn create_block_id_option() -> BlockIdOption {
    BlockIdOption { block_id: Some(TEST_BLOCK_NUMBER.to_string()) }
}

fn create_ui() -> sozo_ui::SozoUi {
    sozo_ui::SozoUi::new(sozo_ui::SozoUiTheme::dark(), sozo_ui::SozoVerbosity::Quiet)
}

// ============================================================================
// Block Command Tests
// ============================================================================

#[tokio::test]
async fn test_block_by_number() {
    let args = BlockArgs {
        block_ids: vec![TEST_BLOCK_NUMBER.to_string()],
        starknet: create_starknet_options(),
        full: false,
        receipts: false,
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get block: {:?}", result.err());
}

#[tokio::test]
async fn test_block_by_hash() {
    let args = BlockArgs {
        block_ids: vec![TEST_BLOCK_HASH.to_string()],
        starknet: create_starknet_options(),
        full: false,
        receipts: false,
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get block by hash: {:?}", result.err());
}

#[tokio::test]
async fn test_block_with_full_txs() {
    let args = BlockArgs {
        block_ids: vec![TEST_BLOCK_NUMBER.to_string()],
        starknet: create_starknet_options(),
        full: true,
        receipts: false,
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get block with full txs: {:?}", result.err());
}

#[tokio::test]
async fn test_block_batch() {
    let args = BlockArgs {
        block_ids: vec![TEST_BLOCK_NUMBER.to_string(), (TEST_BLOCK_NUMBER + 1).to_string()],
        starknet: create_starknet_options(),
        full: false,
        receipts: false,
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to batch get blocks: {:?}", result.err());
}

// ============================================================================
// Transaction Command Tests
// ============================================================================

#[tokio::test]
async fn test_transaction_by_hash() {
    let args = TransactionArgs {
        transaction_hashes: vec![TEST_TX_HASH.to_string()],
        starknet: create_starknet_options(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get transaction: {:?}", result.err());
}

#[tokio::test]
async fn test_transaction_batch() {
    let args = TransactionArgs {
        transaction_hashes: vec![TEST_TX_HASH.to_string(), TEST_TX_HASH_2.to_string()],
        starknet: create_starknet_options(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to batch get transactions: {:?}", result.err());
}

// ============================================================================
// Receipt Command Tests
// ============================================================================

#[tokio::test]
async fn test_receipt() {
    let args = ReceiptArgs {
        transaction_hashes: vec![TEST_TX_HASH.to_string()],
        starknet: create_starknet_options(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get receipt: {:?}", result.err());
}

#[tokio::test]
async fn test_receipt_batch() {
    let args = ReceiptArgs {
        transaction_hashes: vec![TEST_TX_HASH.to_string(), TEST_TX_HASH_2.to_string()],
        starknet: create_starknet_options(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to batch get receipts: {:?}", result.err());
}

// ============================================================================
// Status Command Tests
// ============================================================================

#[tokio::test]
async fn test_status() {
    let args = StatusArgs {
        transaction_hashes: vec![TEST_TX_HASH.to_string()],
        starknet: create_starknet_options(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get status: {:?}", result.err());
}

#[tokio::test]
async fn test_status_batch() {
    let args = StatusArgs {
        transaction_hashes: vec![TEST_TX_HASH.to_string(), TEST_TX_HASH_2.to_string()],
        starknet: create_starknet_options(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to batch get statuses: {:?}", result.err());
}

// ============================================================================
// Balance Command Tests
// ============================================================================

#[tokio::test]
async fn test_balance_strk() {
    let args = BalanceArgs {
        addresses: vec![TEST_ADDRESS.to_string()],
        eth: false,
        token: None,
        starknet: create_starknet_options(),
        block_id: create_block_id_option(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get STRK balance: {:?}", result.err());
}

#[tokio::test]
async fn test_balance_batch() {
    let args = BalanceArgs {
        addresses: vec![TEST_ADDRESS.to_string(), STRK_TOKEN_ADDRESS.to_string()],
        eth: false,
        token: None,
        starknet: create_starknet_options(),
        block_id: create_block_id_option(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to batch get balances: {:?}", result.err());
}

// ============================================================================
// Nonce Command Tests
// ============================================================================

#[tokio::test]
async fn test_nonce() {
    let args = NonceArgs {
        addresses: vec![TEST_ADDRESS.to_string()],
        starknet: create_starknet_options(),
        block_id: create_block_id_option(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get nonce: {:?}", result.err());
}

#[tokio::test]
async fn test_nonce_batch() {
    let args = NonceArgs {
        addresses: vec![TEST_ADDRESS.to_string(), TEST_ADDRESS_2.to_string()],
        starknet: create_starknet_options(),
        block_id: create_block_id_option(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to batch get nonces: {:?}", result.err());
}

// ============================================================================
// ClassHashAt Command Tests
// ============================================================================

#[tokio::test]
async fn test_class_hash_at() {
    let args = ClassHashAtArgs {
        addresses: vec![TEST_ADDRESS.to_string()],
        starknet: create_starknet_options(),
        block_id: create_block_id_option(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get class hash at: {:?}", result.err());
}

#[tokio::test]
async fn test_class_hash_at_batch() {
    let args = ClassHashAtArgs {
        addresses: vec![TEST_ADDRESS.to_string(), STRK_TOKEN_ADDRESS.to_string()],
        starknet: create_starknet_options(),
        block_id: create_block_id_option(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to batch get class hashes at: {:?}", result.err());
}

// ============================================================================
// ClassAt Command Tests
// ============================================================================

#[tokio::test]
async fn test_class_at() {
    let args = ClassAtArgs {
        addresses: vec![TEST_ADDRESS.to_string()],
        starknet: create_starknet_options(),
        block_id: create_block_id_option(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get class at: {:?}", result.err());
}

#[tokio::test]
async fn test_class_at_batch() {
    let args = ClassAtArgs {
        addresses: vec![TEST_ADDRESS.to_string(), STRK_TOKEN_ADDRESS.to_string()],
        starknet: create_starknet_options(),
        block_id: create_block_id_option(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to batch get classes at: {:?}", result.err());
}

// ============================================================================
// ClassByHash Command Tests
// ============================================================================

#[tokio::test]
async fn test_class_by_hash() {
    let args = ClassByHashArgs {
        class_hashes: vec![TEST_CLASS_HASH.to_string()],
        starknet: create_starknet_options(),
        block_id: create_block_id_option(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to get class by hash: {:?}", result.err());
}

#[tokio::test]
async fn test_class_by_hash_batch() {
    let args = ClassByHashArgs {
        class_hashes: vec![TEST_CLASS_HASH.to_string(), STRK_CLASS_HASH.to_string()],
        starknet: create_starknet_options(),
        block_id: create_block_id_option(),
        output: create_output_options(),
    };

    let ui = create_ui();
    let result = args.run(&ui).await;
    assert!(result.is_ok(), "Failed to batch get classes by hash: {:?}", result.err());
}
