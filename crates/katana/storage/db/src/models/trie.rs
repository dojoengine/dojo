use serde::{Deserialize, Serialize};

use crate::codecs::{Decode, Encode};
use crate::error::CodecError;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TrieDatabaseKeyType {
    Trie,
    Flat,
    TrieLog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrieDatabaseKey {
    pub r#type: TrieDatabaseKeyType,
    pub key: [u8; 32],
}

pub type TrieDatabaseValue = [u8; 32];

impl Encode for TrieDatabaseKey {
    type Encoded = [u8; 33];

    fn encode(self) -> Self::Encoded {
        let mut result = [0u8; 33];
        result[0] = self.r#type as u8;
        result[1..].copy_from_slice(&self.key);
        result
    }
}

impl Decode for TrieDatabaseKey {
    fn decode<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        let bytes = bytes.as_ref();
        if bytes.len() != 33 {
            // return Err(CodecError::InvalidLength);
            panic!("Invalid length")
        }

        let r#type = match bytes[0] {
            0 => TrieDatabaseKeyType::Trie,
            1 => TrieDatabaseKeyType::Flat,
            2 => TrieDatabaseKeyType::TrieLog,
            _ => panic!("Invalid trie database key type"),
        };

        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes[1..]);

        Ok(TrieDatabaseKey { r#type, key })
    }
}
