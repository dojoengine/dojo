use katana_primitives::block::{BlockIdOrTag, BlockTag};
use katana_primitives::class::CasmContractClass;
use katana_primitives::state::StateUpdates;
use katana_primitives::Felt;
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use url::Url;

pub mod types;

use types::ContractClass;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] reqwest::Error),

    #[error(transparent)]
    Sequencer(SequencerError),

    #[error("Request rate limited")]
    RateLimited,
}

/// A client for interacting with the Starknet's feeder gateway.
#[derive(Debug, Clone)]
pub struct SequencerGateway {
    base_url: Url,
    client: Client,
}

impl SequencerGateway {
    pub fn new(base_url: Url) -> Self {
        let client = Client::new();
        Self { client, base_url }
    }

    pub fn starknet_mainnet() -> Self {
        Self::new(Url::parse("https://alpha-mainnet.starknet.io/").unwrap())
    }

    pub fn starknet_sepolia() -> Self {
        Self::new(Url::parse("https://alpha-sepolia.starknet.io/").unwrap())
    }
}

impl SequencerGateway {
    pub async fn get_state_update(&self, block_id: BlockIdOrTag) -> Result<StateUpdates, Error> {
        self.feeder_gateway("get_state_update").with_block_id(block_id).send().await
    }

    pub async fn get_state_update_with_block(
        &self,
        block_id: BlockIdOrTag,
    ) -> Result<StateUpdates, Error> {
        self.feeder_gateway("get_state_update")
            .with_query_param("includeBlock", "true")
            .with_block_id(block_id)
            .send()
            .await
    }

    pub async fn get_class(
        &self,
        hash: Felt,
        block_id: BlockIdOrTag,
    ) -> Result<ContractClass, Error> {
        self.feeder_gateway("get_class_by_hash")
            .with_query_param("classHash", &format!("{hash:#x}"))
            .with_block_id(block_id)
            .send()
            .await
    }

    pub async fn get_compiled_class(
        &self,
        hash: Felt,
        block_id: BlockIdOrTag,
    ) -> Result<CasmContractClass, Error> {
        self.feeder_gateway("get_compiled_class_by_class_hash")
            .with_query_param("classHash", &format!("{hash:#x}"))
            .with_block_id(block_id)
            .send()
            .await
    }

    fn feeder_gateway(&self, method: &str) -> RequestBuilder<'_> {
        let mut url = self.base_url.clone();
        url.path_segments_mut().expect("invalid base url").extend(["feeder_gateway", method]);
        RequestBuilder { client: &self.client, url }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Response<T> {
    Data(T),
    Error(SequencerError),
}

#[derive(Debug, Clone)]
struct RequestBuilder<'a> {
    client: &'a Client,
    url: Url,
}

impl<'a> RequestBuilder<'a> {
    fn with_block_id(self, block_id: BlockIdOrTag) -> Self {
        match block_id {
            // latest block is implied, if no block id specified
            BlockIdOrTag::Tag(BlockTag::Latest) => self,
            BlockIdOrTag::Tag(BlockTag::Pending) => self.with_query_param("blockNumber", "pending"),
            BlockIdOrTag::Hash(hash) => self.with_query_param("blockHash", &format!("{hash:#x}")),
            BlockIdOrTag::Number(num) => self.with_query_param("blockNumber", &num.to_string()),
        }
    }

    fn with_query_param(mut self, key: &str, value: &str) -> Self {
        self.url.query_pairs_mut().append_pair(key, value);
        self
    }

    async fn send<T: DeserializeOwned>(self) -> Result<T, Error> {
        let response = self.client.get(self.url).send().await?;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            Err(Error::RateLimited)
        } else {
            match response.json::<Response<T>>().await? {
                Response::Data(data) => Ok(data),
                Response::Error(error) => Err(Error::Sequencer(error)),
            }
        }
    }
}

#[derive(Debug, thiserror::Error, Deserialize)]
#[error("{message} ({code:?})")]
pub struct SequencerError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ErrorCode {
    #[serde(rename = "StarknetErrorCode.BLOCK_NOT_FOUND")]
    BlockNotFound,
    #[serde(rename = "StarknetErrorCode.ENTRY_POINT_NOT_FOUND_IN_CONTRACT")]
    EntryPointNotFoundInContract,
    #[serde(rename = "StarknetErrorCode.INVALID_PROGRAM")]
    InvalidProgram,
    #[serde(rename = "StarknetErrorCode.TRANSACTION_FAILED")]
    TransactionFailed,
    #[serde(rename = "StarknetErrorCode.TRANSACTION_NOT_FOUND")]
    TransactionNotFound,
    #[serde(rename = "StarknetErrorCode.UNINITIALIZED_CONTRACT")]
    UninitializedContract,
    #[serde(rename = "StarkErrorCode.MALFORMED_REQUEST")]
    MalformedRequest,
    #[serde(rename = "StarknetErrorCode.UNDECLARED_CLASS")]
    UndeclaredClass,
    #[serde(rename = "StarknetErrorCode.INVALID_TRANSACTION_NONCE")]
    InvalidTransactionNonce,
    #[serde(rename = "StarknetErrorCode.VALIDATE_FAILURE")]
    ValidateFailure,
    #[serde(rename = "StarknetErrorCode.CLASS_ALREADY_DECLARED")]
    ClassAlreadyDeclared,
    #[serde(rename = "StarknetErrorCode.COMPILATION_FAILED")]
    CompilationFailed,
    #[serde(rename = "StarknetErrorCode.INVALID_COMPILED_CLASS_HASH")]
    InvalidCompiledClassHash,
    #[serde(rename = "StarknetErrorCode.DUPLICATED_TRANSACTION")]
    DuplicatedTransaction,
    #[serde(rename = "StarknetErrorCode.INVALID_CONTRACT_CLASS")]
    InvalidContractClass,
    #[serde(rename = "StarknetErrorCode.DEPRECATED_ENDPOINT")]
    DeprecatedEndpoint,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_block_id() {
        let client = Client::new();
        let base_url = Url::parse("https://example.com/").unwrap();
        let req = RequestBuilder { client: &client, url: base_url };

        // Test pending block
        let pending_url = req.clone().with_block_id(BlockIdOrTag::Tag(BlockTag::Pending)).url;
        assert_eq!(pending_url.query(), Some("blockNumber=pending"));

        // Test block hash
        let hash = Felt::from(123);
        let hash_url = req.clone().with_block_id(BlockIdOrTag::Hash(hash)).url;
        assert_eq!(hash_url.query(), Some("blockHash=0x7b"));

        // Test block number
        let num_url = req.clone().with_block_id(BlockIdOrTag::Number(42)).url;
        assert_eq!(num_url.query(), Some("blockNumber=42"));

        // Test latest block (should have no query params)
        let latest_url = req.with_block_id(BlockIdOrTag::Tag(BlockTag::Latest)).url;
        assert_eq!(latest_url.query(), None);
    }

    #[test]
    fn request_block_id_overwrite() {
        let client = Client::new();
        let base_url = Url::parse("https://example.com/").unwrap();
        let req = RequestBuilder { client: &client, url: base_url };

        let url = req
            .clone()
            .with_block_id(BlockIdOrTag::Tag(BlockTag::Pending))
            .with_block_id(BlockIdOrTag::Number(42))
            .url;

        assert_eq!(url.query(), Some("blockNumber=42"));

        let hash = Felt::from(123);
        let url = req
            .clone()
            .with_block_id(BlockIdOrTag::Hash(hash))
            .with_block_id(BlockIdOrTag::Tag(BlockTag::Pending))
            .url;

        assert_eq!(url.query(), Some("blockNumber=pending"));
    }

    #[test]
    fn multiple_query_params() {
        let client = Client::new();
        let base_url = Url::parse("https://example.com/").unwrap();
        let req = RequestBuilder { client: &client, url: base_url };

        let url = req
            .with_query_param("param1", "value1")
            .with_query_param("param2", "value2")
            .with_query_param("param3", "value3")
            .url;

        let query = url.query().unwrap();
        assert!(query.contains("param1=value1"));
        assert!(query.contains("param2=value2"));
        assert!(query.contains("param3=value3"));
    }
}
