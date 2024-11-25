use katana_primitives::{
    block::{BlockIdOrTag, BlockTag},
    ContractAddress, Felt,
};

pub mod api {
    tonic::include_proto!("starknet");
}

pub mod types {
    tonic::include_proto!("types");
}

pub use api::starknet_server::Starknet as StarknetApi;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Decode(#[from] ::prost::DecodeError),
}

impl TryFrom<types::Felt> for Felt {
    type Error = Error;

    fn try_from(value: types::Felt) -> Result<Self, Self::Error> {
        if value.value.len() > 32 {
            panic!("doesn't fit")
        }

        Ok(Self::from_bytes_be_slice(&value.value))
    }
}

impl From<Felt> for types::Felt {
    fn from(value: Felt) -> Self {
        Self { value: value.to_bytes_be().to_vec() }
    }
}

impl TryFrom<types::Felt> for ContractAddress {
    type Error = Error;

    fn try_from(value: types::Felt) -> Result<Self, Self::Error> {
        Ok(Self::new(Felt::try_from(value)?))
    }
}

impl From<types::BlockTag> for BlockTag {
    fn from(value: types::BlockTag) -> Self {
        match value {
            types::BlockTag::Latest => Self::Latest,
            types::BlockTag::Pending => Self::Pending,
        }
    }
}

impl TryFrom<types::BlockId> for BlockIdOrTag {
    type Error = Error;

    fn try_from(value: types::BlockId) -> Result<Self, Self::Error> {
        use types::block_id::Identifier;

        let Some(id) = value.identifier else { panic!("missing id") };

        match id {
            Identifier::Number(num) => Ok(Self::Number(num)),
            Identifier::Hash(hash) => {
                let felt = Felt::try_from(hash)?;
                Ok(Self::Hash(felt))
            }
            Identifier::Tag(tag) => {
                let tag = types::BlockTag::try_from(tag)?;
                Ok(Self::Tag(BlockTag::from(tag)))
            }
        }
    }
}
