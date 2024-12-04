use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::contract::Nonce;
use katana_primitives::transaction::{
    DeclareTx, DeclareTxV0, DeclareTxV1, DeclareTxV2, DeclareTxV3, DeployAccountTx,
    DeployAccountTxV1, DeployAccountTxV3, DeployTx, InvokeTx, InvokeTxV0, InvokeTxV1, InvokeTxV3,
    L1HandlerTx, Tx, TxHash, TxType, TxWithHash,
};
use katana_primitives::{ContractAddress, Felt};
use serde::Deserialize;

use super::serde_utils::{
    deserialize_optional_u128, deserialize_optional_u64, deserialize_u128, deserialize_u64,
};

#[derive(Debug, Deserialize)]
pub struct ConfirmedTransaction {
    #[serde(rename = "transaction_hash")]
    pub hash: TxHash,
    #[serde(flatten)]
    pub tx: TypedTransaction,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TypedTransaction {
    Deploy(DeployTx),
    Declare(RawDeclareTx),
    L1Handler(RawL1HandlerTx),
    InvokeFunction(RawInvokeTx),
    DeployAccount(RawDeployAccountTx),
}

// We redundantly define the `DataAvailabilityMode` enum here because the serde implementation is
// different from the one in the `katana_primitives` crate. And changing the serde implementation in
// the `katana_primitives` crate would break the database format. So, we have to define the type
// again. But see if we can remove it once we're okay with breaking the database format.
#[derive(Debug)]
pub enum DataAvailabilityMode {
    L1,
    L2,
}

impl<'de> Deserialize<'de> for DataAvailabilityMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(DataAvailabilityMode::L1),
            1 => Ok(DataAvailabilityMode::L2),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid data availability mode; expected 0 or 1 but got {value}"
            ))),
        }
    }
}

// Same reason as `DataAvailabilityMode` above, this struct is also defined because the serde
// implementation of its primitive counterpart is different.
#[derive(Debug, Deserialize)]
pub struct ResourceBounds {
    #[serde(deserialize_with = "deserialize_u64")]
    pub max_amount: u64,
    #[serde(deserialize_with = "deserialize_u128")]
    pub max_price_per_unit: u128,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct ResourceBoundsMapping {
    pub l1_gas: ResourceBounds,
    pub l2_gas: ResourceBounds,
}

#[derive(Debug, Deserialize)]
pub struct RawL1HandlerTx {
    /// The L1 to L2 message nonce.
    pub nonce: Option<Nonce>,
    /// Transaction version.
    pub version: Felt,
    /// The input to the L1 handler function.
    pub calldata: Vec<Felt>,
    /// Contract address of the L1 handler.
    pub contract_address: ContractAddress,
    /// The L1 handler function selector.
    pub entry_point_selector: Felt,
}

#[derive(Debug, Deserialize)]
pub struct RawInvokeTx {
    // Alias for v0 transaction
    #[serde(alias = "contract_address")]
    pub sender_address: ContractAddress,
    // v0 doesn't include nonce
    #[serde(default)]
    pub nonce: Option<Nonce>,
    #[serde(default)]
    pub entry_point_selector: Option<Felt>,
    pub calldata: Vec<Felt>,
    pub signature: Vec<Felt>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_optional_u128")]
    pub max_fee: Option<u128>,
    #[serde(default)]
    pub resource_bounds: Option<ResourceBoundsMapping>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_optional_u64")]
    pub tip: Option<u64>,
    #[serde(default)]
    pub paymaster_data: Option<Vec<Felt>>,
    #[serde(default)]
    pub account_deployment_data: Option<Vec<Felt>>,
    #[serde(default)]
    pub nonce_data_availability_mode: Option<DataAvailabilityMode>,
    #[serde(default)]
    pub fee_data_availability_mode: Option<DataAvailabilityMode>,
    pub version: Felt,
}

#[derive(Debug, Deserialize)]
pub struct RawDeclareTx {
    pub sender_address: ContractAddress,
    pub nonce: Felt,
    pub signature: Vec<Felt>,
    pub class_hash: ClassHash,
    pub compiled_class_hash: Option<CompiledClassHash>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_optional_u128")]
    pub max_fee: Option<u128>,
    #[serde(default)]
    pub resource_bounds: Option<ResourceBoundsMapping>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_optional_u64")]
    pub tip: Option<u64>,
    #[serde(default)]
    pub paymaster_data: Option<Vec<Felt>>,
    #[serde(default)]
    pub account_deployment_data: Option<Vec<Felt>>,
    #[serde(default)]
    pub nonce_data_availability_mode: Option<DataAvailabilityMode>,
    #[serde(default)]
    pub fee_data_availability_mode: Option<DataAvailabilityMode>,
    pub version: Felt,
}

#[derive(Debug, Deserialize)]
pub struct RawDeployAccountTx {
    pub nonce: Nonce,
    pub signature: Vec<Felt>,
    pub class_hash: ClassHash,
    pub contract_address: Option<ContractAddress>,
    pub contract_address_salt: Felt,
    pub constructor_calldata: Vec<Felt>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_optional_u128")]
    pub max_fee: Option<u128>,
    #[serde(default)]
    pub resource_bounds: Option<ResourceBoundsMapping>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_optional_u64")]
    pub tip: Option<u64>,
    #[serde(default)]
    pub paymaster_data: Option<Vec<Felt>>,
    #[serde(default)]
    pub nonce_data_availability_mode: Option<DataAvailabilityMode>,
    #[serde(default)]
    pub fee_data_availability_mode: Option<DataAvailabilityMode>,
    pub version: Felt,
}

#[derive(Debug, thiserror::Error)]
pub enum TxTryFromError {
    #[error("unsupported transaction version; type: {r#type:?}, version: {version:#x}")]
    UnsupportedVersion { r#type: TxType, version: Felt },

    #[error("missing `tip`")]
    MissingTip,

    #[error("missing `paymaster_data`")]
    MissingPaymasterData,

    #[error("missing `entry_point_selector`")]
    MissingEntryPointSelector,

    #[error("missing `nonce`")]
    MissingNonce,

    #[error("missing `max_fee`")]
    MissingMaxFee,

    #[error("missing `resource_bounds`")]
    MissingResourceBounds,

    #[error("missing `account_deployment_data`")]
    MissingAccountDeploymentData,

    #[error("missing nonce `data_availability_mode`")]
    MissingNonceDA,

    #[error("missing fee `data_availability_mode`")]
    MissingFeeDA,

    #[error("missing `compiled_class_hash`")]
    MissingCompiledClassHash,
}

// -- Conversion to Katana primitive types.

impl TryFrom<ConfirmedTransaction> for TxWithHash {
    type Error = TxTryFromError;

    fn try_from(tx: ConfirmedTransaction) -> Result<Self, Self::Error> {
        let transaction = match tx.tx {
            TypedTransaction::Deploy(tx) => Tx::Deploy(tx),
            TypedTransaction::Declare(tx) => Tx::Declare(DeclareTx::try_from(tx)?),
            TypedTransaction::L1Handler(tx) => Tx::L1Handler(L1HandlerTx::from(tx)),
            TypedTransaction::InvokeFunction(tx) => Tx::Invoke(InvokeTx::try_from(tx)?),
            TypedTransaction::DeployAccount(tx) => {
                Tx::DeployAccount(DeployAccountTx::try_from(tx)?)
            }
        };

        Ok(TxWithHash { hash: tx.hash, transaction })
    }
}

impl TryFrom<RawInvokeTx> for InvokeTx {
    type Error = TxTryFromError;

    fn try_from(value: RawInvokeTx) -> Result<Self, Self::Error> {
        if Felt::ZERO == value.version {
            Ok(InvokeTx::V0(InvokeTxV0 {
                calldata: value.calldata,
                signature: value.signature,
                contract_address: value.sender_address,
                max_fee: value.max_fee.ok_or(TxTryFromError::MissingMaxFee)?,
                entry_point_selector: value
                    .entry_point_selector
                    .ok_or(TxTryFromError::MissingEntryPointSelector)?,
            }))
        } else if Felt::ONE == value.version {
            Ok(InvokeTx::V1(InvokeTxV1 {
                chain_id: Default::default(),
                nonce: value.nonce.ok_or(TxTryFromError::MissingNonce)?,
                calldata: value.calldata,
                signature: value.signature,
                max_fee: value.max_fee.ok_or(TxTryFromError::MissingMaxFee)?,
                sender_address: value.sender_address,
            }))
        } else if Felt::THREE == value.version {
            let tip = value.tip.ok_or(TxTryFromError::MissingTip)?;
            let paymaster_data =
                value.paymaster_data.ok_or(TxTryFromError::MissingPaymasterData)?;
            let resource_bounds =
                value.resource_bounds.ok_or(TxTryFromError::MissingResourceBounds)?;
            let account_deployment_data = value
                .account_deployment_data
                .ok_or(TxTryFromError::MissingAccountDeploymentData)?;
            let nonce_data_availability_mode =
                value.nonce_data_availability_mode.ok_or(TxTryFromError::MissingNonceDA)?;
            let fee_data_availability_mode =
                value.fee_data_availability_mode.ok_or(TxTryFromError::MissingFeeDA)?;

            Ok(InvokeTx::V3(InvokeTxV3 {
                tip,
                paymaster_data,
                chain_id: Default::default(),
                nonce: value.nonce.ok_or(TxTryFromError::MissingNonce)?,
                calldata: value.calldata,
                signature: value.signature,
                sender_address: value.sender_address,
                resource_bounds: resource_bounds.into(),
                account_deployment_data,
                fee_data_availability_mode: fee_data_availability_mode.into(),
                nonce_data_availability_mode: nonce_data_availability_mode.into(),
            }))
        } else {
            Err(TxTryFromError::UnsupportedVersion {
                r#type: TxType::Invoke,
                version: value.version,
            })
        }
    }
}

impl TryFrom<RawDeclareTx> for DeclareTx {
    type Error = TxTryFromError;

    fn try_from(value: RawDeclareTx) -> Result<Self, Self::Error> {
        if Felt::ZERO == value.version {
            Ok(DeclareTx::V0(DeclareTxV0 {
                signature: value.signature,
                chain_id: Default::default(),
                class_hash: value.class_hash,
                sender_address: value.sender_address,
                max_fee: value.max_fee.ok_or(TxTryFromError::MissingMaxFee)?,
            }))
        } else if Felt::ONE == value.version {
            Ok(DeclareTx::V1(DeclareTxV1 {
                chain_id: Default::default(),
                sender_address: value.sender_address,
                nonce: value.nonce,
                signature: value.signature,
                class_hash: value.class_hash,
                max_fee: value.max_fee.ok_or(TxTryFromError::MissingMaxFee)?,
            }))
        } else if Felt::TWO == value.version {
            Ok(DeclareTx::V2(DeclareTxV2 {
                chain_id: Default::default(),
                sender_address: value.sender_address,
                nonce: value.nonce,
                signature: value.signature,
                class_hash: value.class_hash,
                compiled_class_hash: value
                    .compiled_class_hash
                    .ok_or(TxTryFromError::MissingCompiledClassHash)?,
                max_fee: value.max_fee.ok_or(TxTryFromError::MissingMaxFee)?,
            }))
        } else if Felt::THREE == value.version {
            let resource_bounds =
                value.resource_bounds.ok_or(TxTryFromError::MissingResourceBounds)?;
            let tip = value.tip.ok_or(TxTryFromError::MissingTip)?;
            let paymaster_data =
                value.paymaster_data.ok_or(TxTryFromError::MissingPaymasterData)?;
            let account_deployment_data = value
                .account_deployment_data
                .ok_or(TxTryFromError::MissingAccountDeploymentData)?;
            let nonce_data_availability_mode =
                value.nonce_data_availability_mode.ok_or(TxTryFromError::MissingNonceDA)?;
            let fee_data_availability_mode =
                value.fee_data_availability_mode.ok_or(TxTryFromError::MissingFeeDA)?;
            let compiled_class_hash =
                value.compiled_class_hash.ok_or(TxTryFromError::MissingCompiledClassHash)?;

            Ok(DeclareTx::V3(DeclareTxV3 {
                chain_id: Default::default(),
                sender_address: value.sender_address,
                nonce: value.nonce,
                signature: value.signature,
                class_hash: value.class_hash,
                compiled_class_hash,
                resource_bounds: resource_bounds.into(),
                tip,
                paymaster_data,
                account_deployment_data,
                nonce_data_availability_mode: nonce_data_availability_mode.into(),
                fee_data_availability_mode: fee_data_availability_mode.into(),
            }))
        } else {
            Err(TxTryFromError::UnsupportedVersion {
                r#type: TxType::Declare,
                version: value.version,
            })
        }
    }
}

impl TryFrom<RawDeployAccountTx> for DeployAccountTx {
    type Error = TxTryFromError;

    fn try_from(value: RawDeployAccountTx) -> Result<Self, Self::Error> {
        if Felt::ONE == value.version {
            Ok(DeployAccountTx::V1(DeployAccountTxV1 {
                chain_id: Default::default(),
                nonce: value.nonce,
                signature: value.signature,
                class_hash: value.class_hash,
                contract_address: value.contract_address.unwrap_or_default(),
                contract_address_salt: value.contract_address_salt,
                constructor_calldata: value.constructor_calldata,
                max_fee: value.max_fee.ok_or(TxTryFromError::MissingMaxFee)?,
            }))
        } else if Felt::THREE == value.version {
            let resource_bounds =
                value.resource_bounds.ok_or(TxTryFromError::MissingResourceBounds)?;
            let tip = value.tip.ok_or(TxTryFromError::MissingTip)?;
            let paymaster_data =
                value.paymaster_data.ok_or(TxTryFromError::MissingPaymasterData)?;
            let nonce_data_availability_mode =
                value.nonce_data_availability_mode.ok_or(TxTryFromError::MissingNonceDA)?;
            let fee_data_availability_mode =
                value.fee_data_availability_mode.ok_or(TxTryFromError::MissingFeeDA)?;

            Ok(DeployAccountTx::V3(DeployAccountTxV3 {
                chain_id: Default::default(),
                nonce: value.nonce,
                signature: value.signature,
                class_hash: value.class_hash,
                contract_address: value.contract_address.unwrap_or_default(),
                contract_address_salt: value.contract_address_salt,
                constructor_calldata: value.constructor_calldata,
                resource_bounds: resource_bounds.into(),
                tip,
                paymaster_data,
                nonce_data_availability_mode: nonce_data_availability_mode.into(),
                fee_data_availability_mode: fee_data_availability_mode.into(),
            }))
        } else {
            Err(TxTryFromError::UnsupportedVersion {
                r#type: TxType::DeployAccount,
                version: value.version,
            })
        }
    }
}

impl From<RawL1HandlerTx> for L1HandlerTx {
    fn from(value: RawL1HandlerTx) -> Self {
        Self {
            version: value.version,
            calldata: value.calldata,
            chain_id: Default::default(),
            message_hash: Default::default(),
            paid_fee_on_l1: Default::default(),
            nonce: value.nonce.unwrap_or_default(),
            contract_address: value.contract_address,
            entry_point_selector: value.entry_point_selector,
        }
    }
}

impl From<DataAvailabilityMode> for katana_primitives::da::DataAvailabilityMode {
    fn from(mode: DataAvailabilityMode) -> Self {
        match mode {
            DataAvailabilityMode::L1 => Self::L1,
            DataAvailabilityMode::L2 => Self::L2,
        }
    }
}

impl From<ResourceBoundsMapping> for katana_primitives::fee::ResourceBoundsMapping {
    fn from(bounds: ResourceBoundsMapping) -> Self {
        Self {
            l1_gas: katana_primitives::fee::ResourceBounds {
                max_amount: bounds.l1_gas.max_amount,
                max_price_per_unit: bounds.l1_gas.max_price_per_unit,
            },
            l2_gas: katana_primitives::fee::ResourceBounds {
                max_amount: bounds.l2_gas.max_amount,
                max_price_per_unit: bounds.l2_gas.max_price_per_unit,
            },
        }
    }
}
