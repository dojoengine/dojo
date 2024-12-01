use katana_primitives::transaction::{
    DeclareTx, DeployAccountTx, DeployTx, InvokeTx, L1HandlerTx, TxHash,
};
use katana_primitives::Felt;
use serde::Deserialize;

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
    #[serde(deserialize_with = "deserialize_declare")]
    Declare(DeclareTx),
    #[serde(deserialize_with = "deserialize_deploy_account")]
    DeployAccount(DeployAccountTx),
    #[serde(deserialize_with = "deserialize_invoke")]
    InvokeFunction(InvokeTx),
    L1Handler(L1HandlerTx),
}

fn deserialize_declare<'de, D>(deserializer: D) -> Result<DeclareTx, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Debug, Deserialize)]
    struct Helper {
        version: TxHash,
        #[serde(flatten)]
        value: serde_json::Value,
    }

    let Helper { version, value } = Helper::deserialize(deserializer)?;

    if version == Felt::ONE {
        let tx = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(DeclareTx::V1(tx))
    } else if version == Felt::TWO {
        let tx = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(DeclareTx::V2(tx))
    } else if version == Felt::THREE {
        let tx = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(DeclareTx::V3(tx))
    } else {
        Err(serde::de::Error::custom(format!("unknown version: {version}")))
    }
}

fn deserialize_deploy_account<'de, D>(deserializer: D) -> Result<DeployAccountTx, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Debug, Deserialize)]
    struct Helper {
        version: TxHash,
        #[serde(flatten)]
        value: serde_json::Value,
    }

    let Helper { version, value } = Helper::deserialize(deserializer)?;

    if version == Felt::ONE {
        let tx = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(DeployAccountTx::V1(tx))
    } else if version == Felt::THREE {
        let tx = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(DeployAccountTx::V3(tx))
    } else {
        Err(serde::de::Error::custom(format!("unknown version: {version}")))
    }
}

fn deserialize_invoke<'de, D>(deserializer: D) -> Result<InvokeTx, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Debug, Deserialize)]
    struct Helper {
        version: TxHash,
        #[serde(flatten)]
        value: serde_json::Value,
    }

    let Helper { version, value } = Helper::deserialize(deserializer)?;

    if version == Felt::ZERO {
        let tx = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(InvokeTx::V0(tx))
    } else if version == Felt::ONE {
        let tx = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(InvokeTx::V1(tx))
    } else if version == Felt::THREE {
        let tx = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(InvokeTx::V3(tx))
    } else {
        Err(serde::de::Error::custom(format!("unknown version: {version}")))
    }
}

#[cfg(test)]
mod tests {

    use katana_primitives::felt;
    use serde_json;

    use super::*;

    #[test]
    fn test_tx_with_hash_deserialization() {
        let json = r#"{
            "type": "INVOKE_FUNCTION",
            "transaction_hash": "0x123",
            "sender_address": "0x456",
            "nonce": "0x1",
            "entry_point_selector": "0x1",
            "calldata": [],
            "signature": [],
            "version": "0x0"
        }"#;

        let tx: ConfirmedTransaction = serde_json::from_str(json).unwrap();

        assert!(matches!(tx.tx, TypedTransaction::InvokeFunction(InvokeTx::V0(..))));
        assert_eq!(tx.hash, felt!("0x123"));

        if let TypedTransaction::InvokeFunction(InvokeTx::V0(v0)) = tx.tx {
            assert_eq!(v0.sender_address, felt!("0x456").into());
            assert_eq!(v0.nonce, felt!("0x1").into());
            assert_eq!(v0.entry_point_selector, felt!("0x1"));
            assert_eq!(v0.calldata.len(), 0);
            assert_eq!(v0.signature.len(), 0);
        } else {
            panic!("wrong variant")
        }
    }
}
