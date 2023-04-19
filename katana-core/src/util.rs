use std::{fs, path::PathBuf};

use blockifier::execution::contract_class::ContractClass;
use starknet_api::{
    calldata,
    core::{calculate_contract_address, ClassHash, ContractAddress},
    hash::StarkFelt,
    stark_felt,
    transaction::{
        Calldata, ContractAddressSalt, DeployAccountTransaction, Fee, TransactionVersion,
    },
};

pub fn get_contract_class(contract_path: &str) -> ContractClass {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), contract_path].iter().collect();
    let raw_contract_class = fs::read_to_string(path).unwrap();
    serde_json::from_str(&raw_contract_class).unwrap()
}

pub fn deploy_account_tx(
    class_hash: &str,
    contract_address_salt: ContractAddressSalt,
    max_fee: Fee,
) -> DeployAccountTransaction {
    let class_hash = ClassHash(stark_felt!(class_hash));
    let deployer_address = ContractAddress::default();
    let contract_address = calculate_contract_address(
        contract_address_salt,
        class_hash,
        &calldata![],
        deployer_address,
    )
    .unwrap();

    DeployAccountTransaction {
        max_fee,
        version: TransactionVersion(stark_felt!(1)),
        class_hash,
        contract_address,
        contract_address_salt,
        ..Default::default()
    }
}
