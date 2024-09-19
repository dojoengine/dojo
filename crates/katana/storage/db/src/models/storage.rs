use katana_primitives::contract::{ContractAddress, StorageKey, StorageValue};

use crate::codecs::{Compress, Decode, Decompress, Encode};
use crate::error::CodecError;

/// Represents a contract storage entry.
///
/// `key` is the subkey for the dupsort table.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct StorageEntry {
    /// The storage key.
    pub key: StorageKey,
    /// The storage value.
    pub value: StorageValue,
}

impl Compress for StorageEntry {
    type Compressed = Vec<u8>;
    fn compress(self) -> Self::Compressed {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.key.to_bytes_be());
        buf.extend_from_slice(&self.value.compress());
        buf
    }
}

impl Decompress for StorageEntry {
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, crate::error::CodecError> {
        let bytes = bytes.as_ref();
        let key = StorageKey::from_bytes_be_slice(&bytes[0..32]);
        let value = StorageValue::decompress(&bytes[32..])?;
        Ok(Self { key, value })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractStorageKey {
    pub contract_address: ContractAddress,
    pub key: StorageKey,
}

impl Encode for ContractStorageKey {
    type Encoded = [u8; 64];
    fn encode(self) -> Self::Encoded {
        let mut buf = [0u8; 64];
        buf[0..32].copy_from_slice(&self.contract_address.encode());
        buf[32..64].copy_from_slice(&self.key.encode());
        buf
    }
}

impl Decode for ContractStorageKey {
    fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        let bytes = bytes.as_ref();
        let contract_address = ContractAddress::decode(&bytes[0..32])?;
        let key = StorageKey::decode(&bytes[32..])?;
        Ok(Self { contract_address, key })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractStorageEntry {
    pub key: ContractStorageKey,
    pub value: StorageValue,
}

impl Compress for ContractStorageEntry {
    type Compressed = Vec<u8>;
    fn compress(self) -> Self::Compressed {
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(self.key.encode().as_ref());
        buf.extend_from_slice(self.value.compress().as_ref());
        buf
    }
}

impl Decompress for ContractStorageEntry {
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        let bytes = bytes.as_ref();
        let key = ContractStorageKey::decode(&bytes[0..64])?;
        let value = StorageValue::decompress(&bytes[64..])?;
        Ok(Self { key, value })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagingCheckpointId {
    SendBlock,
    SendIndex,
    GatherBlock,
    GatherNonce,
}


impl Encode for MessagingCheckpointId {
    type Encoded = [u8; 1];
    fn encode(self) -> Self::Encoded {
        let mut buf = [0u8; 1];
        buf[0] = match self {
            MessagingCheckpointId::SendBlock => 1,
            MessagingCheckpointId::SendIndex => 2,
            MessagingCheckpointId::GatherBlock => 3,
            MessagingCheckpointId::GatherNonce => 4,
        };
        buf
    }
}

impl Decode for MessagingCheckpointId {
    fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        let bytes = bytes.as_ref();
        match bytes[0] {
            1 => Ok(MessagingCheckpointId::SendBlock),
            2 => Ok(MessagingCheckpointId::SendIndex),
            3 => Ok(MessagingCheckpointId::GatherBlock),
            4 => Ok(MessagingCheckpointId::GatherNonce),
            _ => Err(CodecError::Decode("Invalid MessagingCheckpointId".into())),
        }
    }
}


#[cfg(test)]
mod tests {
    use starknet::macros::felt;

    use crate::codecs::{Compress, Decompress};

    #[test]
    fn compress_and_decompress_account_entry() {
        let account_storage_entry = super::ContractStorageEntry {
            key: super::ContractStorageKey {
                contract_address: felt!("0x1234").into(),
                key: felt!("0x111"),
            },
            value: felt!("0x99"),
        };

        let compressed = account_storage_entry.clone().compress();
        let actual_value = super::ContractStorageEntry::decompress(compressed).unwrap();

        assert_eq!(account_storage_entry, actual_value);
    }
}
