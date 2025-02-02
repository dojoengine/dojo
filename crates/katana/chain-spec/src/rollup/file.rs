use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};

use katana_primitives::chain::ChainId;
use katana_primitives::genesis::json::GenesisJson;
use katana_primitives::genesis::Genesis;
use serde::{Deserialize, Serialize};

use super::FeeContract;
use crate::rollup::ChainSpec;
use crate::SettlementLayer;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("OS not supported")]
    UnsupportedOS,

    #[error("config directory not found for chain `{id}`")]
    DirectoryNotFound { id: String },

    #[error("failed to read config file: {0}")]
    ConfigReadError(#[from] toml::ser::Error),

    #[error("failed to write config file: {0}")]
    ConfigWriteError(#[from] toml::de::Error),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    GenesisJson(#[from] katana_primitives::genesis::json::GenesisJsonError),
}

pub fn read(id: &ChainId) -> Result<ChainSpec, Error> {
    read_at(local_dir()?, id)
}

pub fn write(chain_spec: &ChainSpec) -> Result<(), Error> {
    write_at(local_dir()?, chain_spec)
}

/// List all of the available chain configurations.
///
/// This will list only the configurations that are stored in the default local directory. See
/// [`local_dir`].
pub fn list() -> Result<Vec<ChainId>, Error> {
    list_at(local_dir()?)
}

fn read_at<P: AsRef<Path>>(dir: P, id: &ChainId) -> Result<ChainSpec, Error> {
    let dir = ChainConfigDir::open_at(dir, id)?;

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
        fee_contract: chain_spec.fee_contract,
    })
}

fn write_at<P: AsRef<Path>>(dir: P, chain_spec: &ChainSpec) -> Result<(), Error> {
    let dir = ChainConfigDir::create_at(dir, &chain_spec.id)?;

    {
        let cfg = ChainSpecFile {
            id: chain_spec.id,
            settlement: chain_spec.settlement.clone(),
            fee_contract: chain_spec.fee_contract.clone(),
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

fn list_at<P: AsRef<Path>>(dir: P) -> Result<Vec<ChainId>, Error> {
    let mut chains = Vec::new();
    let dir = dir.as_ref();

    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;

            // Ignore entry that is:-
            //
            // - not a directory
            // - name can't be parse as chain id
            // - config file is not found inside the directory
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if let Ok(chain_id) = ChainId::parse(name) {
                        let cs = ChainConfigDir::open(&chain_id).expect("must exist");
                        if cs.config_path().exists() {
                            chains.push(chain_id);
                        }
                    }
                }
            }
        }
    }

    Ok(chains)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ChainSpecFile {
    id: ChainId,
    fee_contract: FeeContract,
    settlement: SettlementLayer,
}

/// The local directory name where the chain configuration files are stored.
const KATANA_LOCAL_DIR: &str = "katana";

// > LOCAL_DIR/$chain_id/
#[derive(Debug, Clone)]
pub struct ChainConfigDir(PathBuf);

impl ChainConfigDir {
    /// Creates a new config directory for the given chain ID.
    ///
    /// The directory will be created at `$LOCAL_DIR/<id>`, where `$LOCAL_DIR` is the path returned
    /// by [`local_dir`].
    ///
    /// This will create the directory if it does not yet exist.
    pub fn create(id: &ChainId) -> Result<Self, Error> {
        Self::create_at(local_dir()?, id)
    }

    /// Opens an existing config directory for the given chain ID.
    ///
    /// The path of the directory is expected to be `$LOCAL_DIR/<id>`, where `$LOCAL_DIR` is the
    /// path returned by [`local_dir`].
    ///
    /// # Errors
    ///
    /// This function will return an error if no directory exists with the given chain ID.
    pub fn open(id: &ChainId) -> Result<Self, Error> {
        Self::open_at(local_dir()?, id)
    }

    pub fn create_at<P: AsRef<Path>>(dir: P, id: &ChainId) -> Result<Self, Error> {
        let id = id.to_string();
        let path = dir.as_ref().join(id);

        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }

        Ok(Self(path))
    }

    pub fn open_at<P: AsRef<Path>>(dir: P, id: &ChainId) -> Result<Self, Error> {
        let id = id.to_string();
        let path = dir.as_ref().join(&id);

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
    use std::path::Path;
    use std::sync::OnceLock;

    use katana_primitives::chain::ChainId;
    use katana_primitives::genesis::Genesis;
    use katana_primitives::ContractAddress;
    use tempfile::TempDir;
    use url::Url;

    use super::Error;
    use crate::rollup::file::{local_dir, ChainConfigDir, KATANA_LOCAL_DIR};
    use crate::rollup::{ChainSpec, FeeContract};
    use crate::SettlementLayer;

    static TEMPDIR: OnceLock<TempDir> = OnceLock::new();

    fn with_temp_dir<T>(f: impl FnOnce(&Path) -> T) -> T {
        f(TEMPDIR.get_or_init(|| tempfile::TempDir::new().unwrap()).path())
    }

    /// Test version of [`super::read`].
    fn read(id: &ChainId) -> Result<ChainSpec, Error> {
        with_temp_dir(|dir| super::read_at(dir, id))
    }

    /// Test version of [`super::write`].
    fn write(chain_spec: &ChainSpec) -> Result<(), Error> {
        with_temp_dir(|dir| super::write_at(dir, chain_spec))
    }

    fn chainspec() -> ChainSpec {
        ChainSpec {
            id: ChainId::default(),
            genesis: Genesis::default(),
            fee_contract: FeeContract { strk: ContractAddress::default() },
            settlement: SettlementLayer::Starknet {
                id: ChainId::default(),
                account: ContractAddress::default(),
                core_contract: ContractAddress::default(),
                rpc_url: Url::parse("http://localhost:5050").expect("valid url"),
            },
        }
    }

    #[test]
    fn test_read_write_chainspec() {
        let chain_spec = chainspec();
        let id = chain_spec.id;

        write(&chain_spec).unwrap();
        let read_spec = read(&id).unwrap();

        assert_eq!(chain_spec.id, read_spec.id);
        assert_eq!(chain_spec.fee_contract, read_spec.fee_contract);
        assert_eq!(chain_spec.settlement, read_spec.settlement);
    }

    #[test]
    fn test_chain_config_dir() {
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
        let dir = local_dir().unwrap();
        assert!(dir.ends_with(KATANA_LOCAL_DIR));
    }

    #[test]
    fn test_config_paths() {
        let chain_id = ChainId::parse("test").unwrap();
        let config_dir = ChainConfigDir::create(&chain_id).unwrap();

        assert!(config_dir.config_path().ends_with("config.toml"));
        assert!(config_dir.genesis_path().ends_with("genesis.json"));
    }

    #[test]
    fn test_list_chain_specs() {
        let dir = tempfile::TempDir::new().unwrap().into_path();

        let listed_chains = super::list_at(&dir).unwrap();
        assert_eq!(listed_chains.len(), 0, "Must be empty initially");

        // Create some dummy chain specs
        let mut chain_specs = Vec::new();
        for i in 1..=3 {
            let mut spec = chainspec();
            // update the chain id to make they're unqiue
            spec.id = ChainId::parse(&format!("chain_{i}")).unwrap();
            chain_specs.push(spec);
        }

        // Write them to disk
        for spec in &chain_specs {
            super::write_at(&dir, spec).unwrap();
        }

        let listed_chains = super::list_at(&dir).unwrap();
        assert_eq!(listed_chains.len(), chain_specs.len());
    }
}
