use std::collections::HashMap;
use std::io::Read;

use anyhow::{anyhow, Ok, Result};
use blockifier::execution::contract_class::{
    ContractClass as InnerContractClass, ContractClassV0 as InnerContractClassV0,
};
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_vm::serde::deserialize_program::ProgramJson;
use serde_json::json;
use starknet::core::types::contract::legacy::{LegacyContractClass, LegacyProgram};
use starknet::core::types::{
    CompressedLegacyContractClass, ContractClass, FieldElement, FlattenedSierraClass,
    LegacyContractEntryPoint, LegacyEntryPointsByType,
};
use starknet_api::deprecated_contract_class::{EntryPoint, EntryPointType};

pub fn legacy_inner_to_rpc_class(
    legacy_contract_class: InnerContractClassV0,
) -> Result<ContractClass> {
    let entry_points_by_type =
        to_rpc_legacy_entry_points_by_type(&legacy_contract_class.entry_points_by_type)
            .expect("Failed to convert entry points");

    let program = {
        let program: ProgramJson = legacy_contract_class.program.clone().into();
        compress(&serde_json::to_vec(&program)?)?
    };

    Ok(ContractClass::Legacy(CompressedLegacyContractClass {
        program,
        entry_points_by_type,
        abi: None,
    }))
}

pub fn rpc_to_inner_class(
    contract_class: &FlattenedSierraClass,
) -> Result<(FieldElement, InnerContractClass)> {
    let class_hash = contract_class.class_hash();

    let value = serde_json::to_value(contract_class)?;
    let contract_class = cairo_lang_starknet::contract_class::ContractClass {
        abi: serde_json::from_value(value["abi"].clone()).ok(),
        sierra_program: serde_json::from_value(value["sierra_program"].clone())?,
        entry_points_by_type: serde_json::from_value(value["entry_points_by_type"].clone())?,
        contract_class_version: serde_json::from_value(value["contract_class_version"].clone())?,
        sierra_program_debug_info: serde_json::from_value(
            value["sierra_program_debug_info"].clone(),
        )
        .ok(),
    };

    let casm_contract = CasmContractClass::from_contract_class(contract_class, true)?;
    Ok((class_hash, InnerContractClass::V1(casm_contract.try_into()?)))
}

pub fn legacy_rpc_to_inner_class(
    compressed_legacy_contract: &CompressedLegacyContractClass,
) -> Result<(FieldElement, InnerContractClass)> {
    let legacy_program_json = decompress(&compressed_legacy_contract.program)?;
    let legacy_program: LegacyProgram = serde_json::from_str(&legacy_program_json)?;

    let flattened = json!({
        "program": legacy_program,
        "abi": compressed_legacy_contract.abi,
        "entry_points_by_type": compressed_legacy_contract.entry_points_by_type,
    });

    let legacy_contract_class: LegacyContractClass = serde_json::from_value(flattened.clone())?;
    let class_hash = legacy_contract_class.class_hash()?;
    let contract_class = serde_json::from_value::<InnerContractClassV0>(flattened)?;

    Ok((class_hash, InnerContractClass::V0(contract_class)))
}

/// Returns a [LegacyEntryPointsByType] (RPC type) from a [EntryPointType] (blockifier type)
fn to_rpc_legacy_entry_points_by_type(
    entries: &HashMap<EntryPointType, Vec<EntryPoint>>,
) -> Result<LegacyEntryPointsByType> {
    fn collect_entry_points(
        entries: &HashMap<EntryPointType, Vec<EntryPoint>>,
        entry_point_type: &EntryPointType,
    ) -> Result<Vec<LegacyContractEntryPoint>> {
        Ok(entries
            .get(entry_point_type)
            .ok_or(anyhow!("Missing {:?} entry point", entry_point_type))?
            .iter()
            .map(|e| LegacyContractEntryPoint {
                offset: e.offset.0 as u64,
                selector: FieldElement::from(e.selector.0),
            })
            .collect::<Vec<_>>())
    }

    let constructor = collect_entry_points(entries, &EntryPointType::Constructor)?;
    let external = collect_entry_points(entries, &EntryPointType::External)?;
    let l1_handler = collect_entry_points(entries, &EntryPointType::L1Handler)?;

    Ok(LegacyEntryPointsByType { constructor, external, l1_handler })
}

/// Returns a compressed vector of bytes
fn compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut gzip_encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    serde_json::to_writer(&mut gzip_encoder, data)?;
    Ok(gzip_encoder.finish()?)
}

fn decompress(data: &[u8]) -> Result<String> {
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut decoded = String::new();
    decoder.read_to_string(&mut decoded)?;
    Ok(decoded)
}
