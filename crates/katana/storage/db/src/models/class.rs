use katana_primitives::contract::CompiledClass;

use crate::codecs::{Compress, Decompress};
use crate::error::CodecError;

impl Compress for CompiledClass {
    type Compressed = Vec<u8>;
    fn compress(self) -> Self::Compressed {
        serde_json::to_vec(&self).unwrap()
    }
}

impl Decompress for CompiledClass {
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        serde_json::from_slice(bytes.as_ref()).map_err(|e| CodecError::Decode(e.to_string()))
    }
}
