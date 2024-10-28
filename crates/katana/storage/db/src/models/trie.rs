use katana_trie::bonsai::ByteVec;
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
