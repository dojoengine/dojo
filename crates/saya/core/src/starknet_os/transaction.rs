//! Transaction conversion into Starknet OS internal transaction type.
//!
//! Transaction internal type python:
//! <https://github.com/starkware-libs/cairo-lang/blob/caba294d82eeeccc3d86a158adb8ba209bf2d8fc/src/starkware/starknet/business_logic/transaction/objects.py#L28>
//! Transaction types:
//! <https://github.com/starkware-libs/cairo-lang/blob/caba294d82eeeccc3d86a158adb8ba209bf2d8fc/src/starkware/starknet/definitions/transaction_type.py#L13>
//!
use std::fmt;

use katana_primitives::transaction::{TxWithHash, Tx, DeclareTx};
use snos::io::InternalTransaction;

use super::felt;

pub fn snos_internal_from_tx(tx_with_hash: &TxWithHash) -> InternalTransaction {
    let mut internal = InternalTransaction {
        hash_value: felt::from_ff(&tx_with_hash.hash),
        ..Default::default()
    };

    match &tx_with_hash.transaction {
        Tx::Invoke(tx) => {
            internal.r#type = TransactionType::InvokeFunction.to_string();
            internal.entry_point_type = Some(EntryPointType::External.to_string());
            internal.version = Some(felt::from_ff(&tx.version));
            internal.nonce = Some(felt::from_ff(&tx.nonce));
            internal.sender_address = Some(felt::from_ff(&(*tx.sender_address)));
            internal.signature = Some(felt::from_ff_vec(&tx.signature));
            internal.calldata = Some(felt::from_ff_vec(&tx.calldata));
            // Entrypoint selector can be retrieved from Call?
        },
        Tx::Declare(tx_e) => {
            match tx_e {
                DeclareTx::V1(tx) => {
                    internal.r#type = TransactionType::Declare.to_string();
                    internal.nonce = Some(felt::from_ff(&tx.nonce));
                    internal.sender_address = Some(felt::from_ff(&(*tx.sender_address)));
                    internal.signature = Some(felt::from_ff_vec(&tx.signature));
                    internal.class_hash = Some(felt::from_ff(&tx.class_hash));
                }
                DeclareTx::V2(tx) => {
                    internal.r#type = TransactionType::Declare.to_string();
                    internal.nonce = Some(felt::from_ff(&tx.nonce));
                    internal.sender_address = Some(felt::from_ff(&(*tx.sender_address)));
                    internal.signature = Some(felt::from_ff_vec(&tx.signature));
                    internal.class_hash = Some(felt::from_ff(&tx.class_hash));
                }
            }
        },
        Tx::L1Handler(tx) => {
            internal.r#type = TransactionType::L1Handler.to_string();
            internal.entry_point_type = Some(EntryPointType::L1Handler.to_string());
            internal.nonce = Some(felt::from_ff(&tx.nonce));
            internal.contract_address = Some(felt::from_ff(&(*tx.contract_address)));
            internal.entry_point_selector = Some(felt::from_ff(&tx.entry_point_selector));
            internal.calldata = Some(felt::from_ff_vec(&tx.calldata));
        },
        Tx::DeployAccount(tx) => {
            internal.r#type = TransactionType::DeployAccount.to_string();
            internal.nonce = Some(felt::from_ff(&tx.nonce));
            internal.contract_address = Some(felt::from_ff(&(*tx.contract_address)));
            internal.contract_address_salt = Some(felt::from_ff(&tx.contract_address_salt));
            internal.class_hash = Some(felt::from_ff(&tx.class_hash));
            internal.constructor_calldata = Some(felt::from_ff_vec(&tx.constructor_calldata));
            internal.signature = Some(felt::from_ff_vec(&tx.signature));
        },
        _ => {}
    };

    internal
}

#[derive(Debug)]
enum TransactionType {
    Declare,
    Deploy,
    DeployAccount,
    InitializeBlockInfo,
    InvokeFunction,
    L1Handler,
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            TransactionType::Declare => "DECLARE",
            TransactionType::Deploy => "DEPLOY",
            TransactionType::DeployAccount => "DEPLOY_ACCOUNT",
            TransactionType::InitializeBlockInfo => "INITIALIZE_BLOCK_INFO",
            TransactionType::InvokeFunction => "INVOKE_FUNCTION",
            TransactionType::L1Handler => "L1_HANDLER",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug)]
enum EntryPointType {
    External,
    L1Handler,
    Constructor,
}

impl fmt::Display for EntryPointType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            EntryPointType::External => "EXTERNAL",
            EntryPointType::L1Handler => "L1_HANDLER",
            EntryPointType::Constructor => "CONSTRUCTOR",
        };
        write!(f, "{}", s)
    }
}
