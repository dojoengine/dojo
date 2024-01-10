use std::collections::HashMap;
use std::io::{Read, Write};

use anyhow::{anyhow, Result};
use blockifier::execution::contract_class::ContractClassV0;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_vm::serde::deserialize_program::ProgramJson;
use serde_json::json;
pub use starknet::core::types::contract::legacy::{LegacyContractClass, LegacyProgram};
pub use starknet::core::types::contract::CompiledClass;
use starknet::core::types::{
    CompressedLegacyContractClass, ContractClass, LegacyContractEntryPoint, LegacyEntryPointsByType,
};
use starknet_api::deprecated_contract_class::{EntryPoint, EntryPointType};

use crate::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, CompiledContractClassV0,
    FlattenedSierraClass,
};
use crate::FieldElement;

mod primitives {
    pub use crate::contract::{CompiledContractClass, ContractAddress, Nonce};
    pub use crate::transaction::{DeclareTx, DeployAccountTx, InvokeTx, L1HandlerTx, Tx};
    pub use crate::FieldElement;
}

/// Converts the legacy inner compiled class type [CompiledContractClassV0] into its RPC equivalent
/// [`ContractClass`].
pub fn legacy_inner_to_rpc_class(
    legacy_contract_class: CompiledContractClassV0,
) -> Result<ContractClass> {
    // Convert [EntryPointType] (blockifier type) into [LegacyEntryPointsByType] (RPC type)
    fn to_rpc_legacy_entry_points_by_type(
        entries: &HashMap<EntryPointType, Vec<EntryPoint>>,
    ) -> Result<LegacyEntryPointsByType> {
        fn collect_entry_points(
            entries: &HashMap<EntryPointType, Vec<EntryPoint>>,
            entry_point_type: &EntryPointType,
        ) -> Result<Vec<LegacyContractEntryPoint>> {
            Ok(entries
                .get(entry_point_type)
                .ok_or(anyhow!("Missing {entry_point_type:?} entry point",))?
                .iter()
                .map(|e| LegacyContractEntryPoint {
                    offset: e.offset.0 as u64,
                    selector: FieldElement::from(e.selector.0),
                })
                .collect::<Vec<_>>())
        }

        Ok(LegacyEntryPointsByType {
            external: collect_entry_points(entries, &EntryPointType::External)?,
            l1_handler: collect_entry_points(entries, &EntryPointType::L1Handler)?,
            constructor: collect_entry_points(entries, &EntryPointType::Constructor)?,
        })
    }

    let entry_points_by_type =
        to_rpc_legacy_entry_points_by_type(&legacy_contract_class.entry_points_by_type)?;

    let program = {
        let program: ProgramJson = legacy_contract_class.program.clone().into();
        compress(&serde_json::to_vec(&program)?)?
    };

    Ok(ContractClass::Legacy(CompressedLegacyContractClass {
        program,
        abi: None,
        entry_points_by_type,
    }))
}

/// Convert the given [`FlattenedSierraClass`] into the inner compiled class type
/// [`CompiledContractClass`] along with its class hashes.
pub fn flattened_sierra_to_compiled_class(
    contract_class: &FlattenedSierraClass,
) -> Result<(ClassHash, CompiledClassHash, CompiledContractClass)> {
    let class_hash = contract_class.class_hash();

    let contract_class = rpc_to_cairo_contract_class(contract_class)?;
    let casm_contract = CasmContractClass::from_contract_class(contract_class, true)?;

    // compute compiled class hash
    let res = serde_json::to_string(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&res)?;

    Ok((
        class_hash,
        compiled_class.class_hash()?,
        CompiledContractClass::V1(casm_contract.try_into()?),
    ))
}

/// Compute the compiled class hash from the given [`FlattenedSierraClass`].
pub fn compiled_class_hash_from_flattened_sierra_class(
    contract_class: &FlattenedSierraClass,
) -> Result<FieldElement> {
    let contract_class = rpc_to_cairo_contract_class(contract_class)?;
    let casm = CasmContractClass::from_contract_class(contract_class, true)?;
    let compiled_class: CompiledClass = serde_json::from_str(&serde_json::to_string(&casm)?)?;
    Ok(compiled_class.class_hash()?)
}

/// Converts a legacy RPC compiled contract class [CompressedLegacyContractClass] type to the inner
/// compiled class type [CompiledContractClass] along with its class hash.
pub fn legacy_rpc_to_inner_compiled_class(
    compressed_legacy_contract: &CompressedLegacyContractClass,
) -> Result<(ClassHash, CompiledContractClass)> {
    let legacy_program_json = decompress(&compressed_legacy_contract.program)?;
    let legacy_program: LegacyProgram = serde_json::from_str(&legacy_program_json)?;

    let flattened = json!({
        "program": legacy_program,
        "abi": compressed_legacy_contract.abi,
        "entry_points_by_type": compressed_legacy_contract.entry_points_by_type,
    });

    let legacy_contract_class: LegacyContractClass = serde_json::from_value(flattened.clone())?;
    let class_hash = legacy_contract_class.class_hash()?;
    let contract_class: ContractClassV0 = serde_json::from_value(flattened)?;

    Ok((class_hash, CompiledContractClass::V0(contract_class)))
}

/// Converts `starknet-rs` RPC [FlattenedSierraClass] type to Cairo's
/// [ContractClass](cairo_lang_starknet::contract_class::ContractClass) type.
fn rpc_to_cairo_contract_class(
    contract_class: &FlattenedSierraClass,
) -> Result<cairo_lang_starknet::contract_class::ContractClass, std::io::Error> {
    let value = serde_json::to_value(contract_class)?;

    Ok(cairo_lang_starknet::contract_class::ContractClass {
        abi: serde_json::from_value(value["abi"].clone()).ok(),
        sierra_program: serde_json::from_value(value["sierra_program"].clone())?,
        entry_points_by_type: serde_json::from_value(value["entry_points_by_type"].clone())?,
        contract_class_version: serde_json::from_value(value["contract_class_version"].clone())?,
        sierra_program_debug_info: serde_json::from_value(
            value["sierra_program_debug_info"].clone(),
        )
        .ok(),
    })
}

fn compress(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut gzip_encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    Write::write_all(&mut gzip_encoder, data)?;
    gzip_encoder.finish()
}

fn decompress(data: &[u8]) -> Result<String, std::io::Error> {
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut decoded = String::new();
    Read::read_to_string(&mut decoder, &mut decoded)?;
    Ok(decoded)
}
