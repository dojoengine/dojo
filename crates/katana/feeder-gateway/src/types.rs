pub use katana_primitives::class::CasmContractClass;
use katana_primitives::class::{LegacyContractClass, SierraContractClass};
use katana_rpc_types::class::ConversionError;
pub use katana_rpc_types::class::RpcSierraContractClass;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ContractClass {
    Class(RpcSierraContractClass),
    Legacy(LegacyContractClass),
}

impl TryFrom<ContractClass> for katana_primitives::class::ContractClass {
    type Error = ConversionError;

    fn try_from(value: ContractClass) -> Result<Self, Self::Error> {
        match value {
            ContractClass::Legacy(class) => Ok(Self::Legacy(class)),
            ContractClass::Class(class) => {
                let class = SierraContractClass::try_from(class)?;
                Ok(Self::Class(class))
            }
        }
    }
}
