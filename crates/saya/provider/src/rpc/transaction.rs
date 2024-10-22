//! Transactions related conversions.
use katana_primitives::chain::ChainId;
use katana_primitives::da::DataAvailabilityMode;
use katana_primitives::fee::{ResourceBounds, ResourceBoundsMapping};
use katana_primitives::transaction::{
    DeclareTx, DeclareTxV1, DeclareTxV2, DeclareTxV3, DeployAccountTx, DeployAccountTxV1,
    DeployAccountTxV3, InvokeTx, InvokeTxV1, InvokeTxV3, L1HandlerTx, Tx, TxWithHash,
};
use num_traits::ToPrimitive;
use starknet::core::types::{
    DeclareTransaction, DeployAccountTransaction, InvokeTransaction, Transaction,
};

use crate::ProviderResult;

pub fn tx_from_rpc(tx_rpc: &Transaction, chain_id: ChainId) -> ProviderResult<TxWithHash> {
    match tx_rpc {
        Transaction::Invoke(tx_e) => match tx_e {
            InvokeTransaction::V0(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: {
                    Tx::Invoke(InvokeTx::V1(InvokeTxV1 {
                        max_fee: tx.max_fee.to_u128().expect("valid u128"),
                        chain_id,
                        calldata: tx.calldata.clone(),
                        signature: tx.signature.clone(),
                        ..Default::default()
                    }))
                },
            }),
            InvokeTransaction::V1(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::Invoke(InvokeTx::V1(InvokeTxV1 {
                    max_fee: tx.max_fee.to_u128().expect("valid u128"),
                    chain_id,
                    calldata: tx.calldata.clone(),
                    signature: tx.signature.clone(),
                    nonce: tx.nonce,
                    sender_address: tx.sender_address.into(),
                })),
            }),
            InvokeTransaction::V3(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::Invoke(InvokeTx::V3(InvokeTxV3 {
                    chain_id,
                    sender_address: tx.sender_address.into(),
                    nonce: tx.nonce,
                    calldata: tx.calldata.clone(),
                    signature: tx.signature.clone(),
                    resource_bounds: from_rpc_resource_bounds(tx.resource_bounds.clone()),
                    tip: tx.tip,
                    paymaster_data: tx.paymaster_data.clone(),
                    account_deployment_data: tx.account_deployment_data.clone(),
                    nonce_data_availability_mode: from_rpc_da_mode(tx.nonce_data_availability_mode),
                    fee_data_availability_mode: from_rpc_da_mode(tx.fee_data_availability_mode),
                })),
            }),
        },
        Transaction::L1Handler(tx) => {
            // Seems we have data loss from only this content from the transaction.
            // The receipt may be required to complete the data.
            // (or use directly the database...)
            Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::L1Handler(L1HandlerTx {
                    nonce: tx.nonce.into(),
                    chain_id,
                    version: 0u8.into(),
                    calldata: tx.calldata.clone(),
                    contract_address: tx.contract_address.into(),
                    entry_point_selector: tx.entry_point_selector,
                    ..Default::default()
                }),
            })
        }
        Transaction::Declare(tx_e) => match tx_e {
            DeclareTransaction::V0(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::Declare(DeclareTx::V1(DeclareTxV1 {
                    max_fee: tx.max_fee.to_u128().expect("valid u128"),
                    chain_id,
                    class_hash: tx.class_hash,
                    signature: tx.signature.clone(),
                    sender_address: tx.sender_address.into(),
                    ..Default::default()
                })),
            }),
            DeclareTransaction::V1(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::Declare(DeclareTx::V1(DeclareTxV1 {
                    nonce: tx.nonce,
                    max_fee: tx.max_fee.to_u128().expect("valid u128"),
                    chain_id,
                    class_hash: tx.class_hash,
                    signature: tx.signature.clone(),
                    sender_address: tx.sender_address.into(),
                })),
            }),
            DeclareTransaction::V2(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::Declare(DeclareTx::V2(DeclareTxV2 {
                    nonce: tx.nonce,
                    max_fee: tx.max_fee.to_u128().expect("valid u128"),
                    chain_id,
                    class_hash: tx.class_hash,
                    signature: tx.signature.clone(),
                    sender_address: tx.sender_address.into(),
                    compiled_class_hash: tx.compiled_class_hash,
                })),
            }),
            DeclareTransaction::V3(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::Declare(DeclareTx::V3(DeclareTxV3 {
                    chain_id,
                    sender_address: tx.sender_address.into(),
                    nonce: tx.nonce,
                    signature: tx.signature.clone(),
                    class_hash: tx.class_hash,
                    compiled_class_hash: tx.compiled_class_hash,
                    resource_bounds: from_rpc_resource_bounds(tx.resource_bounds.clone()),
                    tip: tx.tip,
                    paymaster_data: tx.paymaster_data.clone(),
                    account_deployment_data: tx.account_deployment_data.clone(),
                    nonce_data_availability_mode: from_rpc_da_mode(tx.nonce_data_availability_mode),
                    fee_data_availability_mode: from_rpc_da_mode(tx.fee_data_availability_mode),
                })),
            }),
        },
        Transaction::DeployAccount(tx_e) => match tx_e {
            DeployAccountTransaction::V1(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::DeployAccount(DeployAccountTx::V1(DeployAccountTxV1 {
                    nonce: tx.nonce,
                    max_fee: tx.max_fee.to_u128().expect("valid u128"),
                    chain_id,
                    class_hash: tx.class_hash,
                    signature: tx.signature.clone(),
                    contract_address_salt: tx.contract_address_salt,
                    constructor_calldata: tx.constructor_calldata.clone(),
                    // contract_address field is missing in tx, to be checked.
                    ..Default::default()
                })),
            }),
            DeployAccountTransaction::V3(tx) => Ok(TxWithHash {
                hash: tx.transaction_hash,
                transaction: Tx::DeployAccount(DeployAccountTx::V3(DeployAccountTxV3 {
                    chain_id,
                    nonce: tx.nonce,
                    signature: tx.signature.clone(),
                    class_hash: tx.class_hash,
                    // contract_address field is missing in tx, to be checked.
                    contract_address: Default::default(),
                    contract_address_salt: tx.contract_address_salt,
                    constructor_calldata: tx.constructor_calldata.clone(),
                    resource_bounds: from_rpc_resource_bounds(tx.resource_bounds.clone()),
                    tip: tx.tip,
                    paymaster_data: tx.paymaster_data.clone(),
                    nonce_data_availability_mode: from_rpc_da_mode(tx.nonce_data_availability_mode),
                    fee_data_availability_mode: from_rpc_da_mode(tx.fee_data_availability_mode),
                })),
            }),
        },
        Transaction::Deploy(_) => {
            panic!("Deploy transaction not supported");
        }
    }
}

fn from_rpc_da_mode(mode: starknet::core::types::DataAvailabilityMode) -> DataAvailabilityMode {
    match mode {
        starknet::core::types::DataAvailabilityMode::L1 => DataAvailabilityMode::L1,
        starknet::core::types::DataAvailabilityMode::L2 => DataAvailabilityMode::L2,
    }
}

fn from_rpc_resource_bounds(
    rpc_bounds: starknet::core::types::ResourceBoundsMapping,
) -> ResourceBoundsMapping {
    ResourceBoundsMapping {
        l1_gas: ResourceBounds {
            max_amount: rpc_bounds.l1_gas.max_amount,
            max_price_per_unit: rpc_bounds.l1_gas.max_price_per_unit,
        },
        l2_gas: ResourceBounds {
            max_amount: rpc_bounds.l2_gas.max_amount,
            max_price_per_unit: rpc_bounds.l2_gas.max_price_per_unit,
        },
    }
}
