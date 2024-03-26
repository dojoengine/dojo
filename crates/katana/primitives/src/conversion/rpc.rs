use std::collections::{BTreeMap, HashMap};
use std::io::{self, Read, Write};
use std::mem;

use anyhow::{Context, Result};
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use serde::Deserialize;
use serde_json::json;
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
pub use starknet::core::types::contract::legacy::{LegacyContractClass, LegacyProgram};
use starknet::core::types::contract::legacy::{
    LegacyDebugInfo, LegacyFlowTrackingData, LegacyHint, LegacyIdentifier, LegacyReferenceManager,
};
pub use starknet::core::types::contract::CompiledClass;
use starknet::core::types::{
    CompressedLegacyContractClass, ContractClass, FunctionStateMutability, LegacyContractAbiEntry,
    LegacyContractEntryPoint, LegacyEntryPointsByType, LegacyEventAbiEntry, LegacyEventAbiType,
    LegacyFunctionAbiEntry, LegacyFunctionAbiType, LegacyStructAbiEntry, LegacyStructAbiType,
    LegacyStructMember, LegacyTypedParameter,
};
use starknet_api::deprecated_contract_class::{
    ContractClassAbiEntry, EntryPoint, EntryPointType, TypedParameter,
};

use crate::class::{
    ClassHash, CompiledClassHash, DeprecatedCompiledClass, FlattenedSierraClass,
    SierraCompiledClass, SierraProgram,
};
use crate::FieldElement;

/// Converts the legacy inner compiled class type [DeprecatedCompiledClass] into its RPC equivalent
/// [`ContractClass`].
pub fn legacy_inner_to_rpc_class(
    legacy_contract_class: DeprecatedCompiledClass,
) -> Result<ContractClass> {
    fn to_rpc_entry_points(
        entries: &HashMap<EntryPointType, Vec<EntryPoint>>,
    ) -> Result<LegacyEntryPointsByType> {
        fn collect_entry_points(
            entries: &HashMap<EntryPointType, Vec<EntryPoint>>,
            entry_point_type: &EntryPointType,
        ) -> Result<Vec<LegacyContractEntryPoint>> {
            Ok(entries
                .get(entry_point_type)
                .context(format!("Missing {entry_point_type:?} entry point"))?
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

    fn convert_typed_param(param: Vec<TypedParameter>) -> Vec<LegacyTypedParameter> {
        param
            .into_iter()
            .map(|param| LegacyTypedParameter { name: param.name, r#type: param.r#type })
            .collect()
    }

    fn convert_abi_entry(abi: ContractClassAbiEntry) -> LegacyContractAbiEntry {
        match abi {
            ContractClassAbiEntry::Function(a) => {
                LegacyContractAbiEntry::Function(LegacyFunctionAbiEntry {
                    name: a.name,
                    r#type: LegacyFunctionAbiType::Function,
                    inputs: convert_typed_param(a.inputs),
                    outputs: convert_typed_param(a.outputs),
                    state_mutability: a.state_mutability.map(|_| FunctionStateMutability::View),
                })
            }

            ContractClassAbiEntry::Event(a) => LegacyContractAbiEntry::Event(LegacyEventAbiEntry {
                name: a.name,
                r#type: LegacyEventAbiType::Event,
                data: convert_typed_param(a.data),
                keys: convert_typed_param(a.keys),
            }),

            ContractClassAbiEntry::Constructor(a) => {
                LegacyContractAbiEntry::Function(LegacyFunctionAbiEntry {
                    name: a.name,
                    r#type: LegacyFunctionAbiType::Constructor,
                    inputs: convert_typed_param(a.inputs),
                    outputs: convert_typed_param(a.outputs),
                    state_mutability: a.state_mutability.map(|_| FunctionStateMutability::View),
                })
            }

            ContractClassAbiEntry::Struct(a) => {
                LegacyContractAbiEntry::Struct(LegacyStructAbiEntry {
                    name: a.name,
                    size: a.size as u64,
                    r#type: LegacyStructAbiType::Struct,
                    members: a
                        .members
                        .into_iter()
                        .map(|m| LegacyStructMember {
                            name: m.param.name,
                            offset: m.offset as u64,
                            r#type: m.param.r#type,
                        })
                        .collect(),
                })
            }

            ContractClassAbiEntry::L1Handler(a) => {
                LegacyContractAbiEntry::Function(LegacyFunctionAbiEntry {
                    name: a.name,
                    r#type: LegacyFunctionAbiType::L1Handler,
                    inputs: convert_typed_param(a.inputs),
                    outputs: convert_typed_param(a.outputs),
                    state_mutability: a.state_mutability.map(|_| FunctionStateMutability::View),
                })
            }
        }
    }

    fn convert_abi(abi: Option<Vec<ContractClassAbiEntry>>) -> Option<Vec<LegacyContractAbiEntry>> {
        abi.map(|abi| abi.into_iter().map(convert_abi_entry).collect())
    }

    let abi = convert_abi(legacy_contract_class.abi);
    let program = compress_legacy_program_data(legacy_contract_class.program.clone())?;
    let entry_points_by_type = to_rpc_entry_points(&legacy_contract_class.entry_points_by_type)?;

    Ok(ContractClass::Legacy(CompressedLegacyContractClass { abi, program, entry_points_by_type }))
}

/// Convert the given [FlattenedSierraClass] into the inner compiled class type
/// [CompiledClass](crate::class::CompiledClass) along with its class hashes.
pub fn flattened_sierra_to_compiled_class(
    contract_class: &FlattenedSierraClass,
) -> Result<(ClassHash, CompiledClassHash, crate::class::CompiledClass)> {
    let class_hash = contract_class.class_hash();

    let class = rpc_to_cairo_contract_class(contract_class)?;

    let program = class.extract_sierra_program()?;
    let entry_points_by_type = class.entry_points_by_type.clone();
    let sierra = SierraProgram { program, entry_points_by_type };

    let casm = CasmContractClass::from_contract_class(class, true)?;
    let compiled_hash = FieldElement::from_bytes_be(&casm.compiled_class_hash().to_be_bytes())?;

    let class = crate::class::CompiledClass::Class(SierraCompiledClass { casm, sierra });
    Ok((class_hash, compiled_hash, class))
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
/// compiled class type [CompiledClass](crate::class::CompiledClass) along with its class hash.
pub fn legacy_rpc_to_compiled_class(
    compressed_legacy_contract: &CompressedLegacyContractClass,
) -> Result<(ClassHash, crate::class::CompiledClass)> {
    let class_json = json!({
        "abi": compressed_legacy_contract.abi.clone().unwrap_or_default(),
        "entry_points_by_type": compressed_legacy_contract.entry_points_by_type,
        "program": decompress_legacy_program_data(&compressed_legacy_contract.program)?,
    });

    let deprecated_class: DeprecatedCompiledClass = serde_json::from_value(class_json.clone())?;
    let class_hash = serde_json::from_value::<LegacyContractClass>(class_json)?.class_hash()?;

    Ok((class_hash, crate::class::CompiledClass::Deprecated(deprecated_class)))
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

fn compress_legacy_program_data(
    legacy_program: starknet_api::deprecated_contract_class::Program,
) -> Result<Vec<u8>, io::Error> {
    let bytes = serde_json::to_vec(&legacy_program)?;

    let mut gzip_encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    Write::write_all(&mut gzip_encoder, &bytes)?;
    gzip_encoder.finish()
}

fn decompress_legacy_program_data(data: &[u8]) -> Result<LegacyProgram, io::Error> {
    #[derive(Deserialize)]
    #[allow(unused)]
    struct LegacyAttribute {
        #[serde(default)]
        accessible_scopes: Vec<String>,
        end_pc: u64,
        flow_tracking_data: Option<LegacyFlowTrackingData>,
        name: String,
        start_pc: u64,
        value: String,
    }

    #[repr(transparent)]
    #[derive(Deserialize)]
    #[allow(unused)]
    struct MainScope(String);

    impl Default for MainScope {
        fn default() -> Self {
            Self(String::from("__main__"))
        }
    }

    #[serde_as]
    #[allow(unused)]
    #[derive(Deserialize)]
    struct LegacyProgramJson {
        attributes: Option<Vec<LegacyAttribute>>,
        builtins: Vec<String>,
        compiler_version: Option<String>,
        #[serde_as(as = "Vec<UfeHex>")]
        data: Vec<FieldElement>,
        debug_info: Option<LegacyDebugInfo>,
        hints: BTreeMap<u64, Vec<LegacyHint>>,
        identifiers: BTreeMap<String, LegacyIdentifier>,
        #[serde(default)]
        main_scope: MainScope,
        prime: String,
        reference_manager: LegacyReferenceManager,
    }

    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut decoded = Vec::new();
    Read::read_to_end(&mut decoder, &mut decoded)?;

    // SAFETY: `LegacyProgramJson` MUST maintain same memory layout as `LegacyProgram`. This
    // would only work if the fields are in the same order and have the same size. Though, both
    // types are using default Rust repr, which means there is no guarantee by the compiler that the
    // memory layout of both types will be the same despite comprised of the same fields and
    // types.
    let program: LegacyProgramJson = serde_json::from_slice(&decoded)?;
    let program: LegacyProgram = unsafe { mem::transmute(program) };

    Ok(program)
}

#[cfg(test)]
mod tests {

    use starknet::core::types::ContractClass;

    use super::{legacy_inner_to_rpc_class, legacy_rpc_to_compiled_class};
    use crate::class::{CompiledClass, DeprecatedCompiledClass};
    use crate::genesis::constant::DEFAULT_OZ_ACCOUNT_CONTRACT;
    use crate::utils::class::parse_deprecated_compiled_class;

    #[test]
    fn legacy_rpc_to_inner_and_back() {
        let json = include_str!("../../contracts/compiled/account.json");
        let json = serde_json::from_str(json).unwrap();
        let class: DeprecatedCompiledClass = parse_deprecated_compiled_class(json).unwrap();

        let Ok(ContractClass::Legacy(compressed_legacy_class)) =
            legacy_inner_to_rpc_class(class.clone())
        else {
            panic!("Expected legacy class");
        };

        let (_, converted_class) = legacy_rpc_to_compiled_class(&compressed_legacy_class).unwrap();

        let CompiledClass::Deprecated(converted) = converted_class else { panic!("invalid class") };

        assert_eq!(class.abi, converted.abi);
        assert_eq!(class.program, converted.program);
        assert_eq!(class.entry_points_by_type, converted.entry_points_by_type);
    }

    #[test]
    fn flattened_sierra_class_to_compiled_class() {
        let sierra = DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap();
        assert!(super::flattened_sierra_to_compiled_class(&sierra).is_ok());
    }
}
