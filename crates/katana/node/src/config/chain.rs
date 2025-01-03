use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::Context;
use katana_primitives::genesis::Genesis;
use katana_primitives::ContractAddress;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainConfig {
    // the initialized chain id
    pub id: String,

    // the fee token contract
    //
    // this corresponds to the l1 token contract
    pub fee_token: ContractAddress,

    pub settlement: SettlementLayer,

    pub genesis: Genesis,
}

impl ChainConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(&path)?;
        let cfg = serde_json::from_str::<StoredChainConfig>(&content)?;

        let file = File::open(&cfg.genesis_path).context("failed to open genesis file")?;
        let genesis: Genesis = serde_json::from_reader(BufReader::new(file))?;

        Ok(Self { id: cfg.id, fee_token: cfg.fee_token, settlement: cfg.settlement, genesis })
    }

    pub fn store<P: AsRef<Path>>(self, path: P) -> anyhow::Result<()> {
        let cfg_path = path.as_ref();

        let mut genesis_path = cfg_path.to_path_buf();
        genesis_path.set_file_name("genesis.json");

        let stored = StoredChainConfig {
            id: self.id,
            fee_token: self.fee_token,
            settlement: self.settlement,
            genesis_path,
        };

        serde_json::to_writer_pretty(File::create(cfg_path)?, &stored)?;
        serde_json::to_writer_pretty(File::create(stored.genesis_path)?, &self.genesis)?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementLayer {
    // the account address that was used to initialized the l1 deployments
    pub account: ContractAddress,

    // The id of the settlement chain.
    pub id: String,

    pub rpc_url: Url,

    // - The token that will be used to pay for tx fee in the appchain.
    // - For now, this must be the native token that is used to pay for tx fee in the settlement
    //   chain.
    pub fee_token: ContractAddress,

    // - The bridge contract for bridging the fee token from L1 to the appchain
    // - This will be part of the initialization process.
    pub bridge_contract: ContractAddress,

    // - The core appchain contract used to settlement
    // - This is deployed on the L1
    pub settlement_contract: ContractAddress,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredChainConfig {
    id: String,
    fee_token: ContractAddress,
    settlement: SettlementLayer,
    #[serde(rename = "genesis")]
    genesis_path: PathBuf,
}
