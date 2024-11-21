use futures::io;
use katana_cairo::lang::starknet_classes::contract_class::ContractEntryPoints;
use katana_cairo::lang::utils::bigint::BigUintAsHex;
use katana_cairo::starknet_api::deprecated_contract_class::{
    ContractClassAbiEntry, EntryPoint, EntryPointType, Program as LegacyProgram,
};
use katana_cairo::starknet_api::serde_utils::deserialize_optional_contract_class_abi_entry_vector;
use katana_primitives::class::{ContractClass, LegacyContractClass, SierraContractClass};
use katana_primitives::Felt;
use katana_primitives::{self};
use serde::{Deserialize, Serialize};
use serde_json_pythonic::to_string_pythonic;
use starknet::core::types::{CompressedLegacyContractClass, FlattenedSierraClass};
use std::collections::HashMap;
use std::io::Write;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            RpcContractClass::Class(class) => Ok(Self::Class(SierraContractClass::from(class))),
            RpcContractClass::Legacy(class) => {
                Ok(Self::Legacy(LegacyContractClass::try_from(class)?))
            }
        }
    }
}

// -- SIERRA CLASS

#[derive(Debug, Serialize, Deserialize, Clone)]
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

impl From<RpcSierraContractClass> for SierraContractClass {
    fn from(value: RpcSierraContractClass) -> Self {
        // TODO: convert the abi from sting pythonic

        let program = value
            .sierra_program
            .into_iter()
            .map(|f| BigUintAsHex { value: f.to_biguint() })
            .collect::<Vec<_>>();

        Self {
            abi: None,
            sierra_program: program,
            sierra_program_debug_info: None,
            entry_points_by_type: value.entry_points_by_type,
            contract_class_version: value.contract_class_version,
        }
    }
}

// -- LEGACY CLASS

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcLegacyContractClass {
    /// A base64 representation of the compressed program code
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

fn compress_legacy_program(program: LegacyProgram) -> Result<Vec<u8>, ConversionError> {
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

// Conversion from `starknet-rs` types for convenience

impl TryFrom<FlattenedSierraClass> for RpcSierraContractClass {
    type Error = ConversionError;

    fn try_from(value: FlattenedSierraClass) -> Result<Self, Self::Error> {
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

#[cfg(test)]
mod tests {
    use super::RpcLegacyContractClass;
    use katana_primitives::class::LegacyContractClass;

    #[test]
    fn legacy_rt() {
        let json = include_str!("../../../contracts/build/account.json");
        let class = serde_json::from_str::<LegacyContractClass>(json).unwrap();

        let rpc = RpcLegacyContractClass::try_from(class.clone()).unwrap();
        let rt = LegacyContractClass::try_from(rpc).unwrap();

        assert_eq!(class, rt);
    }

    #[test]
    fn rt() {}
}
