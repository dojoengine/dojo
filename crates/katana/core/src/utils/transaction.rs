use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction as ExecutionTransaction;
use starknet::core::crypto::compute_hash_on_elements;
use starknet::core::types::{
    DeclareTransaction, DeclareTransactionV1, DeclareTransactionV2, DeployAccountTransaction,
    DeployTransaction, FieldElement, InvokeTransaction, InvokeTransactionV0, InvokeTransactionV1,
    L1HandlerTransaction, Transaction as RpcTransaction,
};
use starknet_api::hash::StarkFelt;
use starknet_api::transaction::{
    DeclareTransaction as ApiDeclareTransaction,
    DeployAccountTransaction as ApiDeployAccountTransaction,
    DeployTransaction as ApiDeployTransaction, InvokeTransaction as ApiInvokeTransaction,
    L1HandlerTransaction as ApiL1HandlerTransaction, Transaction as ApiTransaction,
};

const PREFIX_INVOKE: FieldElement = FieldElement::from_mont([
    18443034532770911073,
    18446744073709551615,
    18446744073709551615,
    513398556346534256,
]);

/// Cairo string for "declare"
const PREFIX_DECLARE: FieldElement = FieldElement::from_mont([
    17542456862011667323,
    18446744073709551615,
    18446744073709551615,
    191557713328401194,
]);

/// Cairo string for "deploy_account"
const PREFIX_DEPLOY_ACCOUNT: FieldElement = FieldElement::from_mont([
    3350261884043292318,
    18443211694809419988,
    18446744073709551615,
    461298303000467581,
]);

pub fn compute_deploy_account_v1_transaction_hash(
    contract_address: FieldElement,
    constructor_calldata: &[FieldElement],
    class_hash: FieldElement,
    salt: FieldElement,
    max_fee: FieldElement,
    chain_id: FieldElement,
    nonce: FieldElement,
) -> FieldElement {
    let calldata_to_hash = [&[class_hash, salt], constructor_calldata].concat();

    compute_hash_on_elements(&[
        PREFIX_DEPLOY_ACCOUNT,
        FieldElement::ONE, // version
        contract_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(&calldata_to_hash),
        max_fee,
        chain_id,
        nonce,
    ])
}

pub fn compute_declare_v1_transaction_hash(
    sender_address: FieldElement,
    class_hash: FieldElement,
    max_fee: FieldElement,
    chain_id: FieldElement,
    nonce: FieldElement,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_DECLARE,
        FieldElement::ONE, // version
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(&[class_hash]),
        max_fee,
        chain_id,
        nonce,
    ])
}

pub fn compute_declare_v2_transaction_hash(
    sender_address: FieldElement,
    class_hash: FieldElement,
    max_fee: FieldElement,
    chain_id: FieldElement,
    nonce: FieldElement,
    compiled_class_hash: FieldElement,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_DECLARE,
        FieldElement::TWO, // version
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(&[class_hash]),
        max_fee,
        chain_id,
        nonce,
        compiled_class_hash,
    ])
}

pub fn compute_invoke_v1_transaction_hash(
    sender_address: FieldElement,
    calldata: &[FieldElement],
    max_fee: FieldElement,
    chain_id: FieldElement,
    nonce: FieldElement,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_INVOKE,
        FieldElement::ONE, // version
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(calldata),
        max_fee,
        chain_id,
        nonce,
    ])
}

#[inline]
pub fn stark_felt_to_field_element_array(arr: &[StarkFelt]) -> Vec<FieldElement> {
    arr.iter().map(|e| (*e).into()).collect()
}

pub fn convert_api_to_rpc_tx(transaction: ApiTransaction) -> RpcTransaction {
    match transaction {
        ApiTransaction::Invoke(invoke) => RpcTransaction::Invoke(convert_invoke_to_rpc_tx(invoke)),
        ApiTransaction::Declare(declare) => {
            RpcTransaction::Declare(convert_declare_to_rpc_tx(declare))
        }
        ApiTransaction::DeployAccount(deploy) => {
            RpcTransaction::DeployAccount(convert_deploy_account_to_rpc_tx(deploy))
        }
        ApiTransaction::L1Handler(l1handler) => {
            RpcTransaction::L1Handler(convert_l1_handle_to_rpc(l1handler))
        }
        ApiTransaction::Deploy(deploy) => RpcTransaction::Deploy(convert_deploy_to_rpc(deploy)),
    }
}

fn convert_l1_handle_to_rpc(transaction: ApiL1HandlerTransaction) -> L1HandlerTransaction {
    L1HandlerTransaction {
        transaction_hash: transaction.transaction_hash.0.into(),
        contract_address: (*transaction.contract_address.0.key()).into(),
        nonce: <StarkFelt as Into<FieldElement>>::into(transaction.nonce.0)
            .try_into()
            .expect("able to convert starkfelt to u64"),
        version: <StarkFelt as Into<FieldElement>>::into(transaction.version.0)
            .try_into()
            .expect("able to convert starkfelt to u64"),
        entry_point_selector: transaction.entry_point_selector.0.into(),
        calldata: stark_felt_to_field_element_array(&transaction.calldata.0),
    }
}

fn convert_deploy_to_rpc(transaction: ApiDeployTransaction) -> DeployTransaction {
    DeployTransaction {
        transaction_hash: transaction.transaction_hash.0.into(),
        version: <StarkFelt as Into<FieldElement>>::into(transaction.version.0)
            .try_into()
            .expect("able to convert starkfelt to u64"),
        class_hash: transaction.class_hash.0.into(),
        contract_address_salt: transaction.contract_address_salt.0.into(),
        constructor_calldata: stark_felt_to_field_element_array(
            &transaction.constructor_calldata.0,
        ),
    }
}

fn convert_deploy_account_to_rpc_tx(
    transaction: ApiDeployAccountTransaction,
) -> DeployAccountTransaction {
    DeployAccountTransaction {
        nonce: transaction.nonce.0.into(),
        max_fee: transaction.max_fee.0.into(),
        class_hash: transaction.class_hash.0.into(),
        transaction_hash: transaction.transaction_hash.0.into(),
        contract_address_salt: transaction.contract_address_salt.0.into(),
        constructor_calldata: stark_felt_to_field_element_array(
            &transaction.constructor_calldata.0,
        ),
        signature: stark_felt_to_field_element_array(&transaction.signature.0),
    }
}

fn convert_invoke_to_rpc_tx(transaction: ApiInvokeTransaction) -> InvokeTransaction {
    match transaction {
        ApiInvokeTransaction::V0(tx) => InvokeTransaction::V0(InvokeTransactionV0 {
            nonce: tx.nonce.0.into(),
            max_fee: tx.max_fee.0.into(),
            transaction_hash: tx.transaction_hash.0.into(),
            contract_address: (*tx.sender_address.0.key()).into(),
            entry_point_selector: tx.entry_point_selector.0.into(),
            calldata: stark_felt_to_field_element_array(&tx.calldata.0),
            signature: stark_felt_to_field_element_array(&tx.signature.0),
        }),
        ApiInvokeTransaction::V1(tx) => InvokeTransaction::V1(InvokeTransactionV1 {
            nonce: tx.nonce.0.into(),
            max_fee: tx.max_fee.0.into(),
            transaction_hash: tx.transaction_hash.0.into(),
            sender_address: (*tx.sender_address.0.key()).into(),
            calldata: stark_felt_to_field_element_array(&tx.calldata.0),
            signature: stark_felt_to_field_element_array(&tx.signature.0),
        }),
    }
}

fn convert_declare_to_rpc_tx(transaction: ApiDeclareTransaction) -> DeclareTransaction {
    match transaction {
        ApiDeclareTransaction::V0(tx) | ApiDeclareTransaction::V1(tx) => {
            DeclareTransaction::V1(DeclareTransactionV1 {
                nonce: tx.nonce.0.into(),
                max_fee: tx.max_fee.0.into(),
                class_hash: tx.class_hash.0.into(),
                transaction_hash: tx.transaction_hash.0.into(),
                sender_address: (*tx.sender_address.0.key()).into(),
                signature: stark_felt_to_field_element_array(&tx.signature.0),
            })
        }
        ApiDeclareTransaction::V2(tx) => DeclareTransaction::V2(DeclareTransactionV2 {
            nonce: tx.nonce.0.into(),
            max_fee: tx.max_fee.0.into(),
            class_hash: tx.class_hash.0.into(),
            transaction_hash: tx.transaction_hash.0.into(),
            sender_address: (*tx.sender_address.0.key()).into(),
            compiled_class_hash: tx.compiled_class_hash.0.into(),
            signature: stark_felt_to_field_element_array(&tx.signature.0),
        }),
    }
}

pub fn convert_blockifier_to_api_tx(transaction: &ExecutionTransaction) -> ApiTransaction {
    match transaction {
        ExecutionTransaction::AccountTransaction(tx) => match tx {
            AccountTransaction::Invoke(tx) => ApiTransaction::Invoke(tx.clone()),
            AccountTransaction::Declare(tx) => ApiTransaction::Declare(tx.tx().clone()),
            AccountTransaction::DeployAccount(tx) => ApiTransaction::DeployAccount(tx.clone()),
        },
        ExecutionTransaction::L1HandlerTransaction(tx) => ApiTransaction::L1Handler(tx.tx.clone()),
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::chain_id;

    use super::*;

    #[test]
    fn test_compute_deploy_account_v1_transaction_hash() {
        let contract_address = FieldElement::from_hex_be(
            "0x0617e350ebed9897037bdef9a09af65049b85ed2e4c9604b640f34bffa152149",
        )
        .unwrap();
        let constructor_calldata = vec![
            FieldElement::from_hex_be(
                "0x33434ad846cdd5f23eb73ff09fe6fddd568284a0fb7d1be20ee482f044dabe2",
            )
            .unwrap(),
            FieldElement::from_hex_be(
                "0x79dc0da7c54b95f10aa182ad0a46400db63156920adb65eca2654c0945a463",
            )
            .unwrap(),
            FieldElement::from_hex_be("0x2").unwrap(),
            FieldElement::from_hex_be(
                "0x43a8fbe19d5ace41a2328bb870143241831180eb3c3c48096642d63709c3096",
            )
            .unwrap(),
            FieldElement::from_hex_be("0x0").unwrap(),
        ];
        let class_hash = FieldElement::from_hex_be(
            "0x025ec026985a3bf9d0cc1fe17326b245dfdc3ff89b8fde106542a3ea56c5a918",
        )
        .unwrap();
        let salt = FieldElement::from_hex_be(
            "0x43a8fbe19d5ace41a2328bb870143241831180eb3c3c48096642d63709c3096",
        )
        .unwrap();
        let max_fee = FieldElement::from_hex_be("0x38d7ea4c68000").unwrap();
        let chain_id = chain_id::MAINNET;
        let nonce = FieldElement::ZERO;

        let hash = compute_deploy_account_v1_transaction_hash(
            contract_address,
            &constructor_calldata,
            class_hash,
            salt,
            max_fee,
            chain_id,
            nonce,
        );

        assert_eq!(
            hash,
            FieldElement::from_hex_be(
                "0x3d013d17c20a5db05d5c2e06c948a4e0bf5ea5b851b15137316533ec4788b6b"
            )
            .unwrap()
        );
    }
}
