//! The types used are intentionally chosen so that they can be easily converted from RPC to the
//! internal types without having to rely on intermediary representation.

use std::collections::HashMap;
use std::io::{self, Write};

use katana_cairo::lang::starknet_classes::contract_class::ContractEntryPoints;
use katana_cairo::lang::utils::bigint::BigUintAsHex;
use katana_cairo::starknet_api::deprecated_contract_class::{
    ContractClassAbiEntry, EntryPoint, EntryPointType, Program as LegacyProgram,
};
use katana_cairo::starknet_api::serde_utils::deserialize_optional_contract_class_abi_entry_vector;
use katana_primitives::class::{ContractClass, LegacyContractClass, SierraContractClass};
use katana_primitives::{
    Felt, {self},
};
use serde::{Deserialize, Serialize};
use serde_json_pythonic::to_string_pythonic;
use starknet::core::serde::byte_array::base64;
use starknet::core::types::{CompressedLegacyContractClass, FlattenedSierraClass};

/// RPC representation of the contract class.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcContractClass {
    Class(RpcSierraContractClass),
    Legacy(RpcLegacyContractClass),
}

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    AbiPythonic(#[from] serde_json_pythonic::Error),
}

impl TryFrom<ContractClass> for RpcContractClass {
    type Error = ConversionError;

    fn try_from(value: ContractClass) -> Result<Self, Self::Error> {
        match value {
            ContractClass::Class(class) => {
                Ok(Self::Class(RpcSierraContractClass::try_from(class)?))
            }
            ContractClass::Legacy(class) => {
                Ok(Self::Legacy(RpcLegacyContractClass::try_from(class)?))
            }
        }
    }
}

impl TryFrom<RpcContractClass> for ContractClass {
    type Error = ConversionError;

    fn try_from(value: RpcContractClass) -> Result<Self, Self::Error> {
        match value {
            RpcContractClass::Class(class) => {
                Ok(Self::Class(SierraContractClass::try_from(class)?))
            }
            RpcContractClass::Legacy(class) => {
                Ok(Self::Legacy(LegacyContractClass::try_from(class)?))
            }
        }
    }
}

// -- SIERRA CLASS

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RpcSierraContractClass {
    pub sierra_program: Vec<Felt>,
    pub contract_class_version: String,
    pub entry_points_by_type: ContractEntryPoints,
    pub abi: String,
}

impl TryFrom<SierraContractClass> for RpcSierraContractClass {
    type Error = ConversionError;

    fn try_from(value: SierraContractClass) -> Result<Self, Self::Error> {
        let abi = to_string_pythonic(&value.abi.unwrap_or_default())?;
        let program = value.sierra_program.into_iter().map(|f| f.value.into()).collect::<Vec<_>>();

        Ok(Self {
            abi,
            sierra_program: program,
            entry_points_by_type: value.entry_points_by_type,
            contract_class_version: value.contract_class_version,
        })
    }
}

impl TryFrom<RpcSierraContractClass> for SierraContractClass {
    type Error = ConversionError;

    fn try_from(value: RpcSierraContractClass) -> Result<Self, Self::Error> {
        use katana_cairo::lang::starknet_classes::abi;

        let abi = serde_json::from_str::<Option<abi::Contract>>(&value.abi)?;
        let program = value
            .sierra_program
            .into_iter()
            .map(|f| BigUintAsHex { value: f.to_biguint() })
            .collect::<Vec<_>>();

        Ok(Self {
            abi,
            sierra_program: program,
            sierra_program_debug_info: None,
            entry_points_by_type: value.entry_points_by_type,
            contract_class_version: value.contract_class_version,
        })
    }
}

// -- LEGACY CLASS

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcLegacyContractClass {
    /// A base64 representation of the compressed program code
    #[serde(with = "base64")]
    pub program: Vec<u8>,
    /// The selector of each entry point is a unique identifier in the program.
    pub entry_points_by_type: HashMap<EntryPointType, Vec<EntryPoint>>,
    // Starknet does not verify the abi. If we can't parse it, we set it to None.
    #[serde(default, deserialize_with = "deserialize_optional_contract_class_abi_entry_vector")]
    pub abi: Option<Vec<ContractClassAbiEntry>>,
}

impl TryFrom<LegacyContractClass> for RpcLegacyContractClass {
    type Error = ConversionError;

    fn try_from(value: LegacyContractClass) -> Result<Self, Self::Error> {
        let program = compress_legacy_program(value.program)?;
        Ok(Self { program, abi: value.abi, entry_points_by_type: value.entry_points_by_type })
    }
}

impl TryFrom<RpcLegacyContractClass> for LegacyContractClass {
    type Error = ConversionError;

    fn try_from(value: RpcLegacyContractClass) -> Result<Self, Self::Error> {
        let program = decompress_legacy_program(&value.program)?;
        Ok(Self { program, abi: value.abi, entry_points_by_type: value.entry_points_by_type })
    }
}

fn compress_legacy_program(mut program: LegacyProgram) -> Result<Vec<u8>, ConversionError> {
    // We don't need the debug info in the compressed program.
    program.debug_info = serde_json::to_value::<Option<()>>(None)?;

    let bytes = serde_json::to_vec(&program)?;
    let mut gzip_encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    Write::write_all(&mut gzip_encoder, &bytes)?;
    Ok(gzip_encoder.finish()?)
}

fn decompress_legacy_program(compressed_data: &[u8]) -> Result<LegacyProgram, ConversionError> {
    let mut decoder = flate2::read::GzDecoder::new(compressed_data);
    let mut decompressed = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut decompressed)?;
    Ok(serde_json::from_slice::<LegacyProgram>(&decompressed)?)
}

// Conversion from `starknet-rs` types for convenience.
//
// These are not the most efficient way to convert the types, but they are the most convenient.
// Considering we are not using `starknet-rs` types for the contract class definitions in Katana and
// mainly for utility purposes, these conversions should be avoided from being used in a program
// hot path.

impl TryFrom<starknet::core::types::ContractClass> for RpcContractClass {
    type Error = ConversionError;

    fn try_from(value: starknet::core::types::ContractClass) -> Result<Self, Self::Error> {
        match value {
            starknet::core::types::ContractClass::Legacy(class) => {
                Ok(Self::Legacy(RpcLegacyContractClass::try_from(class)?))
            }
            starknet::core::types::ContractClass::Sierra(class) => {
                Ok(Self::Class(RpcSierraContractClass::try_from(class)?))
            }
        }
    }
}

impl TryFrom<RpcContractClass> for starknet::core::types::ContractClass {
    type Error = ConversionError;

    fn try_from(value: RpcContractClass) -> Result<Self, Self::Error> {
        match value {
            RpcContractClass::Legacy(class) => {
                Ok(Self::Legacy(CompressedLegacyContractClass::try_from(class)?))
            }
            RpcContractClass::Class(class) => {
                Ok(Self::Sierra(FlattenedSierraClass::try_from(class)?))
            }
        }
    }
}

impl TryFrom<FlattenedSierraClass> for RpcSierraContractClass {
    type Error = ConversionError;

    fn try_from(value: FlattenedSierraClass) -> Result<Self, Self::Error> {
        let value = serde_json::to_value(value)?;
        let class = serde_json::from_value::<Self>(value)?;
        Ok(class)
    }
}

impl TryFrom<RpcSierraContractClass> for FlattenedSierraClass {
    type Error = ConversionError;

    fn try_from(value: RpcSierraContractClass) -> Result<Self, Self::Error> {
        let value = serde_json::to_value(value)?;
        let class = serde_json::from_value::<Self>(value)?;
        Ok(class)
    }
}

impl TryFrom<CompressedLegacyContractClass> for RpcLegacyContractClass {
    type Error = ConversionError;

    fn try_from(value: CompressedLegacyContractClass) -> Result<Self, Self::Error> {
        let value = serde_json::to_value(value)?;
        let class = serde_json::from_value::<Self>(value)?;
        Ok(class)
    }
}

impl TryFrom<RpcLegacyContractClass> for CompressedLegacyContractClass {
    type Error = ConversionError;

    fn try_from(value: RpcLegacyContractClass) -> Result<Self, Self::Error> {
        let value = serde_json::to_value(value)?;
        let class = serde_json::from_value::<Self>(value)?;
        Ok(class)
    }
}

#[cfg(test)]
mod tests {
    use katana_primitives::class::{ContractClass, LegacyContractClass, SierraContractClass};
    use starknet::core::types::contract::legacy::LegacyContractClass as StarknetRsLegacyContractClass;
    use starknet::core::types::contract::SierraClass;

    use super::RpcLegacyContractClass;
    use crate::class::RpcSierraContractClass;

    #[test]
    fn rt() {
        let json = include_str!("../../../contracts/build/default_account.json");
        let class = serde_json::from_str::<SierraContractClass>(json).unwrap();

        let rpc = RpcSierraContractClass::try_from(class.clone()).unwrap();
        let rt = SierraContractClass::try_from(rpc).unwrap();

        assert_eq!(class.abi, rt.abi);
        assert_eq!(class.sierra_program, rt.sierra_program);
        assert_eq!(class.entry_points_by_type, rt.entry_points_by_type);
        assert_eq!(class.contract_class_version, rt.contract_class_version);
    }

    #[test]
    fn legacy_rt() {
        let json = include_str!("../../../contracts/build/account.json");
        let class = serde_json::from_str::<LegacyContractClass>(json).unwrap();

        let rpc = RpcLegacyContractClass::try_from(class.clone()).unwrap();
        let rt = LegacyContractClass::try_from(rpc).unwrap();

        assert_eq!(class.abi, rt.abi);
        assert_eq!(class.entry_points_by_type, rt.entry_points_by_type);
        assert_eq!(class.program.builtins, rt.program.builtins);
        assert_eq!(class.program.compiler_version, rt.program.compiler_version);
        assert_eq!(class.program.data, rt.program.data);
        assert_eq!(class.program.hints, rt.program.hints);
        assert_eq!(class.program.identifiers, rt.program.identifiers);
        assert_eq!(class.program.main_scope, rt.program.main_scope);
        assert_eq!(class.program.prime, rt.program.prime);
        assert_eq!(class.program.reference_manager, rt.program.reference_manager);
        // The debug info is stripped when converting to RPC format.
        assert_eq!(serde_json::to_value::<Option<()>>(None).unwrap(), rt.program.debug_info);
    }

    #[test]
    fn rt_with_starknet_rs() {
        let json = include_str!("../../../contracts/build/default_account.json");
        let expected_class = serde_json::from_str::<SierraContractClass>(json).unwrap();

        // -- starknet-rs

        let starknet_rs_class = serde_json::from_str::<SierraClass>(json).unwrap();
        let starknet_rs_hash = starknet_rs_class.class_hash().unwrap();
        let starknet_rpc = starknet_rs_class.flatten().unwrap();

        // -- katana

        let rpc = RpcSierraContractClass::try_from(starknet_rpc).unwrap();
        let class = SierraContractClass::try_from(rpc).unwrap();
        let hash = ContractClass::Class(class.clone()).class_hash().unwrap();

        assert_eq!(starknet_rs_hash, hash);
        assert_eq!(expected_class.abi, class.abi);
        assert_eq!(expected_class.sierra_program, class.sierra_program);
        assert_eq!(expected_class.entry_points_by_type, class.entry_points_by_type);
        assert_eq!(expected_class.contract_class_version, class.contract_class_version);
    }

    #[test]
    fn legacy_rt_with_starknet_rs() {
        use similar_asserts::assert_eq;

        let json = include_str!("../../../contracts/build/erc20.json");
        let expected_class = serde_json::from_str::<LegacyContractClass>(json).unwrap();

        // -- starknet-rs

        let starknet_rs_class =
            serde_json::from_str::<StarknetRsLegacyContractClass>(json).unwrap();
        let starknet_rs_hash = starknet_rs_class.class_hash().unwrap();
        let starknet_rs_rpc = starknet_rs_class.compress().unwrap();

        let json = serde_json::to_string(&starknet_rs_rpc).unwrap();

        // -- katana

        let rpc = serde_json::from_str::<RpcLegacyContractClass>(&json).unwrap();
        let class = LegacyContractClass::try_from(rpc).unwrap();
        let hash = ContractClass::Legacy(class.clone()).class_hash().unwrap();

        assert_eq!(starknet_rs_hash, hash);
        assert_eq!(expected_class.abi, class.abi);
        assert_eq!(expected_class.entry_points_by_type, class.entry_points_by_type);
        assert_eq!(expected_class.program.builtins, class.program.builtins);
        assert_eq!(expected_class.program.compiler_version, class.program.compiler_version);
        assert_eq!(expected_class.program.data, class.program.data);
        assert_eq!(expected_class.program.hints, class.program.hints);
        assert_eq!(expected_class.program.identifiers, class.program.identifiers);
        assert_eq!(expected_class.program.main_scope, class.program.main_scope);
        assert_eq!(expected_class.program.prime, class.program.prime);
        assert_eq!(expected_class.program.reference_manager, class.program.reference_manager);
        // The debug info is stripped when converting to RPC format.
        assert_eq!(serde_json::to_value::<Option<()>>(None).unwrap(), class.program.debug_info);
    }
}
