use katana_trie::bonsai::ByteVec;
use serde::{Deserialize, Serialize};

use crate::codecs::{Compress, Decode, Decompress, Encode};
use crate::error::CodecError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrieHistoryEntry {
    pub key: TrieDatabaseKey,
    pub value: TrieDatabaseValue,
}

impl Compress for TrieHistoryEntry {
    type Compressed = Vec<u8>;

    fn compress(self) -> Self::Compressed {
        let mut buf = Vec::new();
        buf.extend(self.key.encode());
        buf.extend(self.value.compress());
        buf
    }
}

impl Decompress for TrieHistoryEntry {
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        let bytes = bytes.as_ref();

        let key = TrieDatabaseKey::decode(bytes)?;
        // first byte is the key type, second byte is the actual key length
        let key_bytes_length = 1 + 1 + key.key.len();
        let value = TrieDatabaseValue::decompress(&bytes[key_bytes_length..])?;

        Ok(Self { key, value })
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TrieDatabaseKeyType {
    Trie = 0,
    Flat,
    TrieLog,
}

#[derive(Debug, thiserror::Error)]
#[error("unknown trie key type: {0}")]
pub struct TrieDatabaseKeyTypeTryFromError(u8);

impl TryFrom<u8> for TrieDatabaseKeyType {
    type Error = TrieDatabaseKeyTypeTryFromError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Trie),
            1 => Ok(Self::Flat),
            2 => Ok(Self::TrieLog),
            invalid => Err(TrieDatabaseKeyTypeTryFromError(invalid)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TrieDatabaseKey {
    pub r#type: TrieDatabaseKeyType,
    pub key: Vec<u8>,
}

pub type TrieDatabaseValue = ByteVec;

impl Encode for TrieDatabaseKey {
    type Encoded = Vec<u8>;

    fn encode(self) -> Self::Encoded {
        let mut encoded = Vec::new();
        encoded.push(self.r#type as u8);
        encoded.push(self.key.len() as u8); // Encode key length
        encoded.extend(self.key);
        encoded
    }
}

impl Decode for TrieDatabaseKey {
    fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        let bytes = bytes.as_ref();

        if bytes.len() < 2 {
            // Need at least type and length bytes
            panic!("empty buffer")
        }

        let r#type =
            TrieDatabaseKeyType::try_from(bytes[0]).expect("Invalid trie database key type");
        let key_len = bytes[1] as usize;

        if bytes.len() < 2 + key_len {
            panic!("Buffer too short for key length");
        }

        let key = bytes[2..2 + key_len].to_vec();

        Ok(TrieDatabaseKey { r#type, key })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trie_key_roundtrip() {
        let key = TrieDatabaseKey { r#type: TrieDatabaseKeyType::Trie, key: vec![1, 2, 3] };
        let encoded = key.clone().encode();
        let decoded = TrieDatabaseKey::decode(encoded).unwrap();
        assert_eq!(key, decoded);
    }

    #[test]
    fn test_flat_key_roundtrip() {
        let key = TrieDatabaseKey { r#type: TrieDatabaseKeyType::Flat, key: vec![4, 5, 6] };
        let encoded = key.clone().encode();
        let decoded = TrieDatabaseKey::decode(encoded).unwrap();
        assert_eq!(key, decoded);
    }

    #[test]
    fn test_trielog_key_roundtrip() {
        let key = TrieDatabaseKey { r#type: TrieDatabaseKeyType::TrieLog, key: vec![7, 8, 9] };
        let encoded = key.clone().encode();
        let decoded = TrieDatabaseKey::decode(encoded).unwrap();
        assert_eq!(key, decoded);
    }
}
