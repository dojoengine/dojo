use std::collections::BTreeMap;
use std::sync::Arc;

use ::blockifier::transaction::transaction_execution::Transaction;
use ::blockifier::transaction::transactions::{DeployAccountTransaction, InvokeTransaction};
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transactions::{DeclareTransaction, L1HandlerTransaction};
use katana_primitives::transaction::{
    DeclareTx, DeployAccountTx, ExecutableTx, ExecutableTxWithHash, InvokeTx,
};
use starknet_api::core::{ClassHash, CompiledClassHash, EntryPointSelector, Nonce};
use starknet_api::data_availability::DataAvailabilityMode;
use starknet_api::transaction::{
    AccountDeploymentData, Calldata, ContractAddressSalt,
    DeclareTransaction as ApiDeclareTransaction, DeclareTransactionV0V1, DeclareTransactionV2,
    DeclareTransactionV3, DeployAccountTransaction as ApiDeployAccountTransaction,
    DeployAccountTransactionV1, DeployAccountTransactionV3, Fee,
    InvokeTransaction as ApiInvokeTransaction, PaymasterData, Resource, ResourceBounds,
    ResourceBoundsMapping, Tip, TransactionHash, TransactionSignature, TransactionVersion,
};

/// A newtype wrapper for execution transaction used in `blockifier`.
pub struct BlockifierTx(pub(super) ::blockifier::transaction::transaction_execution::Transaction);

impl From<ExecutableTxWithHash> for BlockifierTx {
    fn from(value: ExecutableTxWithHash) -> Self {
        let hash = value.hash;

        let tx = match value.transaction {
            ExecutableTx::Invoke(tx) => match tx {
                InvokeTx::V1(tx) => {
                    let calldata = tx.calldata.into_iter().map(|f| f.into()).collect();
                    let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                    Transaction::AccountTransaction(AccountTransaction::Invoke(InvokeTransaction {
                        tx: ApiInvokeTransaction::V1(
                            starknet_api::transaction::InvokeTransactionV1 {
                                max_fee: Fee(tx.max_fee),
                                nonce: Nonce(tx.nonce.into()),
                                sender_address: tx.sender_address.into(),
                                signature: TransactionSignature(signature),
                                calldata: Calldata(Arc::new(calldata)),
                            },
                        ),
                        tx_hash: TransactionHash(hash.into()),
                        only_query: false,
                    }))
                }

                InvokeTx::V3(tx) => {
                    let calldata = tx.calldata.into_iter().map(|f| f.into()).collect();
                    let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                    let paymaster_data = tx.paymaster_data.into_iter().map(|f| f.into()).collect();
                    let account_deploy_data =
                        tx.account_deployment_data.into_iter().map(|f| f.into()).collect();
                    let fee_data_availability_mode = to_api_da_mode(tx.fee_data_availability_mode);
                    let nonce_data_availability_mode =
                        to_api_da_mode(tx.nonce_data_availability_mode);

                    Transaction::AccountTransaction(AccountTransaction::Invoke(InvokeTransaction {
                        tx: ApiInvokeTransaction::V3(
                            starknet_api::transaction::InvokeTransactionV3 {
                                tip: Tip(tx.tip),
                                nonce: Nonce(tx.nonce.into()),
                                sender_address: tx.sender_address.into(),
                                signature: TransactionSignature(signature),
                                calldata: Calldata(Arc::new(calldata)),
                                paymaster_data: PaymasterData(paymaster_data),
                                account_deployment_data: AccountDeploymentData(account_deploy_data),
                                fee_data_availability_mode,
                                nonce_data_availability_mode,
                                resource_bounds: to_api_resource_bounds(tx.resource_bounds),
                            },
                        ),
                        tx_hash: TransactionHash(hash.into()),
                        only_query: false,
                    }))
                }
            },

            ExecutableTx::DeployAccount(tx) => match tx {
                DeployAccountTx::V1(tx) => {
                    let calldata = tx.constructor_calldata.into_iter().map(|f| f.into()).collect();
                    let signature = tx.signature.into_iter().map(|f| f.into()).collect();
                    let salt = ContractAddressSalt(tx.contract_address_salt.into());

                    Transaction::AccountTransaction(AccountTransaction::DeployAccount(
                        DeployAccountTransaction {
                            contract_address: tx.contract_address.into(),
                            tx: ApiDeployAccountTransaction::V1(DeployAccountTransactionV1 {
                                max_fee: Fee(tx.max_fee),
                                nonce: Nonce(tx.nonce.into()),
                                signature: TransactionSignature(signature),
                                class_hash: ClassHash(tx.class_hash.into()),
                                constructor_calldata: Calldata(Arc::new(calldata)),
                                contract_address_salt: salt,
                            }),
                            tx_hash: TransactionHash(hash.into()),
                            only_query: false,
                        },
                    ))
                }

                DeployAccountTx::V3(tx) => {
                    let calldata = tx.constructor_calldata.into_iter().map(|f| f.into()).collect();
                    let signature = tx.signature.into_iter().map(|f| f.into()).collect();
                    let salt = ContractAddressSalt(tx.contract_address_salt.into());

                    let paymaster_data = tx.paymaster_data.into_iter().map(|f| f.into()).collect();
                    let fee_data_availability_mode = to_api_da_mode(tx.fee_data_availability_mode);
                    let nonce_data_availability_mode =
                        to_api_da_mode(tx.nonce_data_availability_mode);

                    Transaction::AccountTransaction(AccountTransaction::DeployAccount(
                        DeployAccountTransaction {
                            contract_address: tx.contract_address.into(),
                            tx: ApiDeployAccountTransaction::V3(DeployAccountTransactionV3 {
                                tip: Tip(tx.tip),
                                nonce: Nonce(tx.nonce.into()),
                                signature: TransactionSignature(signature),
                                class_hash: ClassHash(tx.class_hash.into()),
                                constructor_calldata: Calldata(Arc::new(calldata)),
                                contract_address_salt: salt,
                                paymaster_data: PaymasterData(paymaster_data),
                                fee_data_availability_mode,
                                nonce_data_availability_mode,
                                resource_bounds: to_api_resource_bounds(tx.resource_bounds),
                            }),
                            tx_hash: TransactionHash(hash.into()),
                            only_query: false,
                        },
                    ))
                }
            },

            ExecutableTx::Declare(tx) => {
                let contract_class = tx.compiled_class;

                let tx = match tx.transaction {
                    DeclareTx::V1(tx) => {
                        let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                        ApiDeclareTransaction::V1(DeclareTransactionV0V1 {
                            max_fee: Fee(tx.max_fee),
                            nonce: Nonce(tx.nonce.into()),
                            sender_address: tx.sender_address.into(),
                            signature: TransactionSignature(signature),
                            class_hash: ClassHash(tx.class_hash.into()),
                        })
                    }

                    DeclareTx::V2(tx) => {
                        let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                        ApiDeclareTransaction::V2(DeclareTransactionV2 {
                            max_fee: Fee(tx.max_fee),
                            nonce: Nonce(tx.nonce.into()),
                            sender_address: tx.sender_address.into(),
                            signature: TransactionSignature(signature),
                            class_hash: ClassHash(tx.class_hash.into()),
                            compiled_class_hash: CompiledClassHash(tx.compiled_class_hash.into()),
                        })
                    }

                    DeclareTx::V3(tx) => {
                        let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                        let paymaster_data =
                            tx.paymaster_data.into_iter().map(|f| f.into()).collect();
                        let fee_data_availability_mode =
                            to_api_da_mode(tx.fee_data_availability_mode);
                        let nonce_data_availability_mode =
                            to_api_da_mode(tx.nonce_data_availability_mode);
                        let account_deploy_data =
                            tx.account_deployment_data.into_iter().map(|f| f.into()).collect();

                        ApiDeclareTransaction::V3(DeclareTransactionV3 {
                            tip: Tip(tx.tip),
                            nonce: Nonce(tx.nonce.into()),
                            sender_address: tx.sender_address.into(),
                            signature: TransactionSignature(signature),
                            class_hash: ClassHash(tx.class_hash.into()),
                            account_deployment_data: AccountDeploymentData(account_deploy_data),
                            compiled_class_hash: CompiledClassHash(tx.compiled_class_hash.into()),
                            paymaster_data: PaymasterData(paymaster_data),
                            fee_data_availability_mode,
                            nonce_data_availability_mode,
                            resource_bounds: to_api_resource_bounds(tx.resource_bounds),
                        })
                    }
                };

                let tx = DeclareTransaction::new(tx, TransactionHash(hash.into()), contract_class)
                    .expect("class mismatch");
                Transaction::AccountTransaction(AccountTransaction::Declare(tx))
            }

            ExecutableTx::L1Handler(tx) => {
                let calldata = tx.calldata.into_iter().map(|f| f.into()).collect();

                Transaction::L1HandlerTransaction(L1HandlerTransaction {
                    paid_fee_on_l1: Fee(tx.paid_fee_on_l1),
                    tx: starknet_api::transaction::L1HandlerTransaction {
                        nonce: Nonce(tx.nonce.into()),
                        calldata: Calldata(Arc::new(calldata)),
                        version: TransactionVersion(1u128.into()),
                        contract_address: tx.contract_address.into(),
                        entry_point_selector: EntryPointSelector(tx.entry_point_selector.into()),
                    },
                    tx_hash: TransactionHash(hash.into()),
                })
            }
        };

        Self(tx)
    }
}

fn to_api_da_mode(mode: starknet::core::types::DataAvailabilityMode) -> DataAvailabilityMode {
    match mode {
        starknet::core::types::DataAvailabilityMode::L1 => DataAvailabilityMode::L1,
        starknet::core::types::DataAvailabilityMode::L2 => DataAvailabilityMode::L2,
    }
}

fn to_api_resource_bounds(
    resource_bounds: starknet::core::types::ResourceBoundsMapping,
) -> ResourceBoundsMapping {
    let l1_gas = ResourceBounds {
        max_amount: resource_bounds.l1_gas.max_amount,
        max_price_per_unit: resource_bounds.l1_gas.max_price_per_unit,
    };

    let l2_gas = ResourceBounds {
        max_amount: resource_bounds.l2_gas.max_amount,
        max_price_per_unit: resource_bounds.l2_gas.max_price_per_unit,
    };

    ResourceBoundsMapping(BTreeMap::from([(Resource::L1Gas, l1_gas), (Resource::L2Gas, l2_gas)]))
}
