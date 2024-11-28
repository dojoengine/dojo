use katana_primitives::class::LegacyContractClass;
pub use katana_rpc_types::class::RpcSierraContractClass;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ContractClass {
    Class(RpcSierraContractClass),
    Legacy(LegacyContractClass),
}

pub type CompiledClass = katana_primitives::class::CasmContractClass;
