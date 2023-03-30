use std::env::{self, current_dir};
use std::fmt::Display;
use std::fs;
use std::str::FromStr;

use anyhow::Context;
use async_trait::async_trait;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_project::WorldConfig;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;
use starknet::core::types::contract::CompiledClass;
use starknet::core::types::FieldElement;
use starknet::core::utils::get_storage_var_address;
use starknet::providers::jsonrpc::models::{BlockId, BlockTag};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
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

#[async_trait]
trait ResolveRemote {
    async fn resolve_remote(&mut self, rpc: &JsonRpcClient<HttpTransport>) -> anyhow::Result<()>;
}

struct Contract {
    name: String,
    address: Option<FieldElement>,
    local: FieldElement,
    remote: Option<FieldElement>,
}

#[async_trait]
impl ResolveRemote for Contract {
    async fn resolve_remote(
        self: &mut Contract,
        rpc: &JsonRpcClient<HttpTransport>,
    ) -> anyhow::Result<()> {
        if let Some(address) = self.address {
            if let Ok(remote_class_hash) =
                rpc.get_class_hash_at(&BlockId::Tag(BlockTag::Latest), address).await
            {
                self.remote = Some(remote_class_hash);
            }
        }

        Ok(())
    }
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
    world: FieldElement,
    name: String,
    local: FieldElement,
    remote: Option<FieldElement>,
}

#[async_trait]
impl ResolveRemote for Class {
    async fn resolve_remote(
        self: &mut Class,
        rpc: &JsonRpcClient<HttpTransport>,
    ) -> anyhow::Result<()> {
        if !matches!(self.name.as_str(), "Indexer" | "Store") {
            let remote_class_hash = rpc
                .get_storage_at(
                    self.world,
                    get_storage_var_address(&self.name.to_lowercase(), &[]).unwrap(),
                    &BlockId::Tag(BlockTag::Latest),
                )
                .await?;
            self.remote = Some(remote_class_hash);
            return Ok(());
        }

        let remote_class_hash = rpc
            .get_storage_at(
                self.world,
                get_storage_var_address("indexer", &[FieldElement::from_str(&self.name).unwrap()])
                    .unwrap(),
                &BlockId::Tag(BlockTag::Latest),
            )
            .await?;
        self.remote = Some(remote_class_hash);
        Ok(())
    }
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
    rpc: JsonRpcClient<HttpTransport>,
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
        let rpc_client = JsonRpcClient::new(HttpTransport::new(
            Url::parse("https://starknet-goerli.cartridge.gg/").unwrap(),
        ));

        let manifest_path = source_dir.join("Scarb.toml");
        let config = Config::builder(manifest_path)
            .ui_verbosity(Verbosity::Verbose)
            .log_filter_directive(env::var_os("SCARB_LOG"))
            .build()
            .unwrap();
        let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
            eprintln!("error: {}", err);
            std::process::exit(1);
        });
        let world_config =
            WorldConfig::from_workspace(&ws).unwrap_or_else(|_| WorldConfig::default());

        let mut world: Option<Contract> = None;
        let mut executor: Option<Contract> = None;
        let mut indexer: Option<Class> = None;
        let mut store: Option<Class> = None;

        let mut contracts = vec![];
        let mut components = vec![];
        let mut systems = vec![];

        // Read the directory
        let entries = fs::read_dir(source_dir.join("target/release")).unwrap_or_else(|error| {
            panic!("Problem reading source directory: {:?}", error);
        });

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            if !file_name_str.ends_with(".json") {
                continue;
            }

            let name = file_name_str.split('_').last().unwrap().to_string();
            let contract_class = serde_json::from_reader(fs::File::open(entry.path()).unwrap())
                .unwrap_or_else(|error| {
                    panic!("Problem parsing {} artifact: {:?}", file_name_str, error);
                });

            let casm_contract = CasmContractClass::from_contract_class(contract_class, true)
                .with_context(|| "Compilation failed.")?;
            let res = serde_json::to_string_pretty(&casm_contract)
                .with_context(|| "Casm contract Serialization failed.")?;

            let compiled_class: CompiledClass =
                serde_json::from_str(res.as_str()).unwrap_or_else(|error| {
                    panic!("Problem parsing {} artifact: {:?}", file_name_str, error);
                });

            let local = compiled_class
                .class_hash()
                .with_context(|| "Casm contract Serialization failed.")?;

            if name.ends_with("Component.json") {
                components.push(Class {
                    world: world_config.address.unwrap(),
                    name: name.strip_suffix("Component.json").unwrap().to_string(),
                    local,
                    remote: None,
                });
            } else if name.ends_with("System.json") {
                systems.push(Class {
                    world: world_config.address.unwrap(),
                    name: name.strip_suffix("System.json").unwrap().to_string(),
                    local,
                    remote: None,
                });
            } else {
                let name = name.strip_suffix(".json").unwrap().to_string();
                match name.as_str() {
                    "World" => {
                        world = Some(Contract {
                            name,
                            local,
                            remote: None,
                            address: world_config.address,
                        })
                    }
                    "Executor" => {
                        executor = Some(Contract { name, local, remote: None, address: None })
                    }
                    "Indexer" => {
                        indexer = Some(Class {
                            world: world_config.address.unwrap(),
                            name,
                            local,
                            remote: None,
                        })
                    }
                    "Store" => {
                        store = Some(Class {
                            world: world_config.address.unwrap(),
                            name,
                            local,
                            remote: None,
                        })
                    }
                    _ => contracts.push(Class {
                        world: world_config.address.unwrap(),
                        name,
                        local,
                        remote: None,
                    }),
                }
            };
        }

        let mut world = World {
            rpc: rpc_client,
            world: world.unwrap_or_else(|| {
                panic!("World contract not found. Did you include `dojo` as a dependency?");
            }),
            executor: executor.unwrap_or_else(|| {
                panic!("Executor contract not found. Did you include `dojo` as a dependency?");
            }),
            indexer: indexer.unwrap_or_else(|| {
                panic!("Indexer contract not found. Did you include `dojo` as a dependency?");
            }),
            store: store.unwrap_or_else(|| {
                panic!("Store contract not found. Did you include `dojo` as a dependency?");
            }),
            contracts,
            components,
            systems,
        };

        world.resolve_remote().await?;

        Ok(world)
    }

    async fn resolve_remote(self: &mut World) -> anyhow::Result<()> {
        self.world.resolve_remote(&self.rpc).await?;
        Ok(())
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
