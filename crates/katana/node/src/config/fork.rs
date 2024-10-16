use katana_primitives::block::BlockHashOrNumber;
use starknet::providers::Url;

/// Node forking configurations.
#[derive(Debug, Clone)]
pub struct ForkingConfig {
    /// The JSON-RPC URL of the network to fork from.
    pub url: Url,
    /// The block number to fork from. If `None`, the latest block will be used.
    pub block: Option<BlockHashOrNumber>,
}
