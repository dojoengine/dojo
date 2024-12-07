use katana_trie::bonsai::ByteVec;
use serde::{Deserialize, Serialize};

use crate::codecs::{Decode, Encode};
use crate::error::CodecError;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TrieDatabaseKeyType {
    Trie = 0,
    Flat,
    TrieLog,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
        encoded.extend(self.key);
        encoded
    }
}

impl Decode for TrieDatabaseKey {
    fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        let bytes = bytes.as_ref();
        if bytes.is_empty() {
            panic!("emptyy buffer")
        }

        let r#type = match bytes[0] {
            0 => TrieDatabaseKeyType::Trie,
            1 => TrieDatabaseKeyType::Flat,
            2 => TrieDatabaseKeyType::TrieLog,
            _ => panic!("Invalid trie database key type"),
        };

        let key = bytes[1..].to_vec();

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
