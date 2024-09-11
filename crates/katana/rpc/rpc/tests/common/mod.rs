use std::fs::File;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use katana_cairo::lang::starknet_classes::casm_contract_class::CasmContractClass;
use katana_cairo::lang::starknet_classes::contract_class::ContractClass;
use katana_primitives::conversion::rpc::CompiledClass;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::{Call, Felt, FlattenedSierraClass};
use starknet::core::utils::get_selector_from_name;

pub fn prepare_contract_declaration_params(
    artifact_path: &PathBuf,
) -> Result<(FlattenedSierraClass, Felt)> {
    let flattened_class = get_flattened_class(artifact_path)
        .map_err(|e| anyhow!("error flattening the contract class: {e}"))?;
    let compiled_class_hash = get_compiled_class_hash(artifact_path)
        .map_err(|e| anyhow!("error computing compiled class hash: {e}"))?;
    Ok((flattened_class, compiled_class_hash))
}

fn get_flattened_class(artifact_path: &PathBuf) -> Result<FlattenedSierraClass> {
    let file = File::open(artifact_path)?;
    let contract_artifact: SierraClass = serde_json::from_reader(&file)?;
    Ok(contract_artifact.flatten()?)
}

fn get_compiled_class_hash(artifact_path: &PathBuf) -> Result<Felt> {
    let file = File::open(artifact_path)?;
    let casm_contract_class: ContractClass = serde_json::from_reader(file)?;
    let casm_contract =
        CasmContractClass::from_contract_class(casm_contract_class, true, usize::MAX)
            .map_err(|e| anyhow!("CasmContractClass from ContractClass error: {e}"))?;
    let res = serde_json::to_string_pretty(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&res)?;
    Ok(compiled_class.class_hash()?)
}

// TODO: not sure why this function is not seen as used
// as prepare_contract_declaration_params is.
#[allow(dead_code)]
pub fn build_deploy_cairo1_contract_call(class_hash: Felt, salt: Felt) -> Call {
    let constructor_calldata = vec![Felt::from(1_u32), Felt::from(2_u32)];

    let calldata = [
        vec![
            class_hash,                             // class hash
            salt,                                   // salt
            Felt::ZERO,                             // unique
            Felt::from(constructor_calldata.len()), // constructor calldata len
        ],
        constructor_calldata.clone(),
    ]
    .concat();

    Call {
        calldata,
        // devnet UDC address
        to: Felt::from_hex("0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf")
            .unwrap(),
        selector: get_selector_from_name("deployContract").unwrap(),
    }
}

/// Splits a Felt into two Felts, representing its lower and upper 128 bits.
#[allow(unused)]
pub fn split_felt(felt: Felt) -> (Felt, Felt) {
    let low: Felt = (felt.to_biguint() & Felt::from(u128::MAX).to_biguint()).into();
    let high = felt.to_biguint() >> 128;
    (low, Felt::from(high))
}
