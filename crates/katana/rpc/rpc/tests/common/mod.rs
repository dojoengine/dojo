use std::fs::File;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract_class::ContractClass;
use katana_primitives::conversion::rpc::CompiledClass;
use starknet::accounts::Call;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::{FieldElement, FlattenedSierraClass};
use starknet::core::utils::get_selector_from_name;

pub fn prepare_contract_declaration_params(
    artifact_path: &PathBuf,
) -> Result<(FlattenedSierraClass, FieldElement)> {
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

fn get_compiled_class_hash(artifact_path: &PathBuf) -> Result<FieldElement> {
    let file = File::open(artifact_path)?;
    let casm_contract_class: ContractClass = serde_json::from_reader(file)?;
    let casm_contract = CasmContractClass::from_contract_class(casm_contract_class, true)
        .map_err(|e| anyhow!("CasmContractClass from ContractClass error: {e}"))?;
    let res = serde_json::to_string_pretty(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&res)?;
    Ok(compiled_class.class_hash()?)
}

// TODO: not sure why this function is not seen as used
// as prepare_contract_declaration_params is.
#[allow(dead_code)]
pub fn build_deploy_cairo1_contract_call(class_hash: FieldElement, salt: FieldElement) -> Call {
    let constructor_calldata = vec![FieldElement::from(1_u32), FieldElement::from(2_u32)];

    let calldata = [
        vec![
            class_hash,                                     // class hash
            salt,                                           // salt
            FieldElement::ZERO,                             // unique
            FieldElement::from(constructor_calldata.len()), // constructor calldata len
        ],
        constructor_calldata.clone(),
    ]
    .concat();

    Call {
        calldata,
        // devnet UDC address
        to: FieldElement::from_hex_be(
            "0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf",
        )
        .unwrap(),
        selector: get_selector_from_name("deployContract").unwrap(),
    }
}
