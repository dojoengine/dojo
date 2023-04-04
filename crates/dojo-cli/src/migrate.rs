use std::env::{self, current_dir};
use std::fmt::Display;

use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_lang::manifest::Manifest;
use dojo_project::WorldConfig;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;
use starknet::core::types::FieldElement;
use url::Url;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,

    #[clap(short, long, help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,
}

#[tokio::main]
pub async fn run(args: MigrateArgs) -> anyhow::Result<()> {
    let source_dir = match args.path {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                let mut current_path = current_dir().unwrap();
                current_path.push(path);
                Utf8PathBuf::from_path_buf(current_path).unwrap()
            }
        }
        None => Utf8PathBuf::from_path_buf(current_dir().unwrap()).unwrap(),
    };

    let world = World::from_path(source_dir).await?;

    println!("{world}");

    Ok(())
}

struct Contract {
    name: String,
    address: Option<FieldElement>,
    local: FieldElement,
    remote: Option<FieldElement>,
}

impl Display for Contract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;
        if let Some(address) = self.address {
            writeln!(f, "   Address: 0x{:x}", address)?;
        }
        writeln!(f, "   Local: 0x{:x}", self.local)?;

        if let Some(remote) = self.remote {
            writeln!(f, "   Remote: 0x{:x}", remote)?;
        }

        Ok(())
    }
}

struct Class {
    #[allow(unused)]
    world: FieldElement,
    name: String,
    local: FieldElement,
    remote: Option<FieldElement>,
}

impl Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;
        writeln!(f, "   Local: 0x{:x}", self.local)?;

        if let Some(remote) = self.remote {
            writeln!(f, "   Remote: 0x{:x}", remote)?;
        }

        Ok(())
    }
}

struct World {
    world: Contract,
    executor: Contract,
    indexer: Class,
    store: Class,
    contracts: Vec<Class>,
    components: Vec<Class>,
    systems: Vec<Class>,
}

impl World {
    async fn from_path(source_dir: Utf8PathBuf) -> anyhow::Result<World> {
        let url = Url::parse("https://starknet-goerli.cartridge.gg/").unwrap();

        let manifest_path = source_dir.join("Scarb.toml");
        let config = Config::builder(manifest_path)
            .ui_verbosity(Verbosity::Verbose)
            .log_filter_directive(env::var_os("SCARB_LOG"))
            .build()
            .unwrap();
        let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
            eprintln!("error: {err}");
            std::process::exit(1);
        });
        let world_config = WorldConfig::from_workspace(&ws).unwrap_or_default();

        let local_manifest =
            Manifest::load_from_path(source_dir.join("target/release/manifest.json"))?;

        let remote_manifest = Manifest::from_remote(url, &local_manifest, &world_config)
            .await
            .map_err(|e| anyhow!("Problem creating remote manifest: {e}"))?;

        let mut systems = vec![];
        let mut contracts = vec![];
        let mut components = vec![];

        for system in local_manifest.systems {
            systems.push(Class {
                world: world_config.address.unwrap(),
                // because the name returns by the `name` method of a system contract is without the 'System' suffix
                name: system.name.strip_suffix("System").unwrap_or(&system.name).to_string(),
                local: system.class_hash,
                remote: remote_manifest
                    .systems
                    .iter()
                    .find(|e| e.name == system.name)
                    .map(|s| s.class_hash),
            });
        }

        for component in local_manifest.components {
            components.push(Class {
                world: world_config.address.unwrap(),
                name: component.name.to_string(),
                local: component.class_hash,
                remote: remote_manifest
                    .components
                    .iter()
                    .find(|e| e.name == component.name)
                    .map(|s| s.class_hash),
            });
        }

        for contract in local_manifest.contracts {
            contracts.push(Class {
                world: world_config.address.unwrap(),
                name: contract.name.to_string(),
                local: contract.class_hash,
                remote: None,
            });
        }

        Ok(World {
            world: Contract {
                name: "World".into(),
                address: world_config.address.clone(),
                local: local_manifest.world.unwrap(),
                remote: remote_manifest.world,
            },
            executor: Contract {
                name: "Executor".into(),
                address: None,
                local: local_manifest.world.unwrap(),
                remote: remote_manifest.world,
            },
            indexer: Class {
                world: world_config.address.unwrap(),
                name: "Indexer".into(),
                local: local_manifest.indexer.unwrap(),
                remote: remote_manifest.indexer,
            },
            store: Class {
                world: world_config.address.unwrap(),
                name: "Store".into(),
                local: local_manifest.store.unwrap(),
                remote: remote_manifest.store,
            },
            systems,
            contracts,
            components,
        })
    }
}

impl Display for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.world)?;
        writeln!(f, "{}", self.executor)?;
        writeln!(f, "{}", self.store)?;
        writeln!(f, "{}", self.indexer)?;

        for component in &self.components {
            writeln!(f, "{}", component)?;
        }

        for system in &self.systems {
            writeln!(f, "{}", system)?;
        }

        for contract in &self.contracts {
            writeln!(f, "{}", contract)?;
        }

        Ok(())
    }
}
