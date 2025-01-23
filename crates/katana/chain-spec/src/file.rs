use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;

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

    #[error("failed to read config file: {0}")]
    ConfigReadError(#[from] toml::ser::Error),

    #[error("failed to write config file: {0}")]
    ConfigWriteError(#[from] toml::de::Error),
}

pub fn read(id: &ChainId) -> Result<ChainSpec, Error> {
    let dir = ChainConfigDir::open(id)?;

    let chain_spec: ChainSpecFile = {
        let content = std::fs::read_to_string(dir.config_path())?;
        toml::from_str(&content)?
    };

    let genesis: Genesis = {
        let file = BufReader::new(File::open(dir.genesis_path())?);
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

pub fn write(chain_spec: &ChainSpec) -> Result<(), Error> {
    let dir = ChainConfigDir::create(&chain_spec.id)?;

    dbg!(&dir);

    {
        let cfg = ChainSpecFile {
            id: chain_spec.id,
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
#[serde(rename_all = "kebab-case")]
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
        let path = local_dir()?.join(id);

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
        let path = local_dir()?.join(&id);

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

#[cfg(test)]
mod tests {
    use super::*;

    // To make sure the path returned by `local_dir` is always the same across
    // testes and is created inside of a temp dir
    fn init() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path();

        #[cfg(target_os = "linux")]
        if std::env::var("XDG_CONFIG_HOME").is_err() {
            std::env::set_var("XDG_CONFIG_HOME", path);
        }

        #[cfg(target_os = "macos")]
        if std::env::var("HOME").is_err() {
            std::env::set_var("HOME", path);
        }
    }

    #[test]
    fn test_read_write_chainspec() {
        init();

        let chain_spec = ChainSpec::default();
        let id = chain_spec.id;

        write(&chain_spec).unwrap();
        let read_spec = read(&id).unwrap();

        assert_eq!(chain_spec.id, read_spec.id);
        assert_eq!(chain_spec.fee_contracts, read_spec.fee_contracts);
        assert_eq!(chain_spec.settlement, read_spec.settlement);
    }

    #[test]
    fn test_chain_config_dir() {
        init();

        let chain_id = ChainId::parse("test").unwrap();

        // Test creation
        let config_dir = ChainConfigDir::create(&chain_id).unwrap();
        assert!(config_dir.0.exists());

        // Test opening existing dir
        let opened_dir = ChainConfigDir::open(&chain_id).unwrap();
        assert_eq!(config_dir.0, opened_dir.0);

        // Test opening non-existent dir
        let bad_id = ChainId::parse("nonexistent").unwrap();
        assert!(matches!(ChainConfigDir::open(&bad_id), Err(Error::DirectoryNotFound { .. })));
    }

    #[test]
    fn test_local_dir() {
        init();

        let dir = local_dir().unwrap();
        assert!(dir.ends_with(KATANA_LOCAL_DIR));
    }

    #[test]
    fn test_config_paths() {
        init();

        let chain_id = ChainId::parse("test").unwrap();
        let config_dir = ChainConfigDir::create(&chain_id).unwrap();

        assert!(config_dir.config_path().ends_with("config.toml"));
        assert!(config_dir.genesis_path().ends_with("genesis.json"));
    }
}
