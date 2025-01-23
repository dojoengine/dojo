use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};

use katana_primitives::chain::ChainId;
use katana_primitives::genesis::json::GenesisJson;
use katana_primitives::genesis::Genesis;
use serde::{Deserialize, Serialize};

use crate::{ChainSpec, FeeContracts, SettlementLayer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("OS not supported")]
    UnsupportedOS,

    #[error("Config directory not found for chain `{id}`")]
    DirectoryNotFound { id: String },

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    GenesisJson(#[from] katana_primitives::genesis::json::GenesisJsonError),

    #[error("failed to serialize config file: {0}")]
    ConfigSerializeError(#[from] toml::ser::Error),
}

pub fn read<P: AsRef<Path>>(id: &ChainId) -> Result<ChainSpec, Error> {
    let dir = ChainConfigDir::open(id)?;

    let chain_spec: ChainSpecFile = {
        let file = BufReader::new(File::open(&dir.config_path())?);
        serde_json::from_reader(file).map_err(io::Error::from)?
    };

    let genesis: Genesis = {
        let file = BufReader::new(File::open(&dir.genesis_path())?);
        let json: GenesisJson = serde_json::from_reader(file).map_err(io::Error::from)?;
        Genesis::try_from(json)?
    };

    Ok(ChainSpec {
        genesis,
        id: chain_spec.id,
        settlement: chain_spec.settlement,
        fee_contracts: chain_spec.fee_contracts,
    })
}

pub fn write<P: AsRef<Path>>(chain_spec: &ChainSpec) -> Result<(), Error> {
    let dir = ChainConfigDir::create(&chain_spec.id)?;

    {
        let cfg = ChainSpecFile {
            id: chain_spec.id.clone(),
            settlement: chain_spec.settlement.clone(),
            fee_contracts: chain_spec.fee_contracts.clone(),
        };

        let content = toml::to_string_pretty(&cfg)?;
        std::fs::write(dir.config_path(), &content)?;
    }

    {
        let genesis_json = GenesisJson::try_from(chain_spec.genesis.clone())?;
        let file = BufWriter::new(File::create(dir.genesis_path())?);
        serde_json::to_writer_pretty(file, &genesis_json).map_err(io::Error::from)?;
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct ChainSpecFile {
    id: ChainId,
    fee_contracts: FeeContracts,
    #[serde(skip_serializing_if = "Option::is_none")]
    settlement: Option<SettlementLayer>,
}

/// The local directory name where the chain configuration files are stored.
const KATANA_LOCAL_DIR: &str = "katana";

// > LOCAL_DIR/$chain_id/
#[derive(Debug, Clone)]
pub struct ChainConfigDir(PathBuf);

impl ChainConfigDir {
    /// Create a new config directory for the given chain ID.
    ///
    /// This will create the directory if it does not yet exist.
    pub fn create(id: &ChainId) -> Result<Self, Error> {
        let id = id.to_string();
        let path = local_dir()?.join(KATANA_LOCAL_DIR).join(id);

        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }

        Ok(Self(path))
    }

    /// Open an existing config directory for the given chain ID.
    ///
    /// This will return an error if the no config directory exists for the given chain ID.
    pub fn open(id: &ChainId) -> Result<Self, Error> {
        let id = id.to_string();
        let path = local_dir()?.join(KATANA_LOCAL_DIR).join(&id);

        if !path.exists() {
            return Err(Error::DirectoryNotFound { id: id.clone() });
        }

        Ok(Self(path))
    }

    /// Get the path to the config file for this chain.
    ///
    /// > $LOCAL_DIR/$chain_id/config.toml
    pub fn config_path(&self) -> PathBuf {
        self.0.join("config").with_extension("toml")
    }

    /// Get the path to the genesis file for this chain.
    ///
    /// > $LOCAL_DIR/$chain_id/genesis.json
    pub fn genesis_path(&self) -> PathBuf {
        self.0.join("genesis").with_extension("json")
    }
}

/// ```
/// | -------- | --------------------------------------------- |
/// | Platform | Path                                          |
/// | -------- | --------------------------------------------- |
/// | Linux    | `$XDG_CONFIG_HOME` or `$HOME`/.config/katana  |
/// | macOS    | `$HOME`/Library/Application Support/katana    |
/// | Windows  | `{FOLDERID_LocalAppData}`/katana              |
/// | -------- | --------------------------------------------- |
/// ```
pub fn local_dir() -> Result<PathBuf, Error> {
    Ok(dirs::config_local_dir().ok_or(Error::UnsupportedOS)?.join(KATANA_LOCAL_DIR))
}
