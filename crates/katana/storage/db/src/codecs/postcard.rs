use katana_primitives::block::Header;
use katana_primitives::contract::{ContractAddress, GenericContractInfo, SierraClass};
use katana_primitives::receipt::Receipt;
use katana_primitives::transaction::Tx;
use katana_primitives::FieldElement;

use super::{Compress, Decompress};
use crate::error::CodecError;
use crate::models::block::StoredBlockBodyIndices;
use crate::models::contract::StoredContractClass;

macro_rules! impl_compress_and_decompress_for_table_values {
    ($($name:ty),*) => {
        $(
            impl Compress for $name {
                type Compressed = Vec<u8>;
                fn compress(self) -> Self::Compressed {
                    postcard::to_stdvec(&self).unwrap()
                }
            }

            impl Decompress for $name {
                fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, crate::error::CodecError> {
                    postcard::from_bytes(bytes.as_ref()).map_err(|e| CodecError::Decode(e.to_string()))
                }
            }
        )*
    }
}

impl_compress_and_decompress_for_table_values!(
    u64,
    Tx,
    Header,
    Receipt,
    SierraClass,
    FieldElement,
    ContractAddress,
    StoredContractClass,
    GenericContractInfo,
    StoredBlockBodyIndices
);
