use core::fmt;

use anyhow::Result;
use clap::Subcommand;
use scarb::core::{Config, Package, Workspace};
use tracing::info_span;

pub(crate) mod build;
pub(crate) mod call;
pub(crate) mod calldata_decoder;
pub(crate) mod clean;
pub(crate) mod execute;
pub(crate) mod hash;
pub(crate) mod init;
pub(crate) mod inspect;
pub(crate) mod migrate;
pub(crate) mod options;
pub(crate) mod test;

use build::BuildArgs;
use call::CallArgs;
use clean::CleanArgs;
use execute::ExecuteArgs;
use hash::HashArgs;
use init::InitArgs;
use inspect::InspectArgs;
use migrate::MigrateArgs;
use test::TestArgs;

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Build the world, generating the necessary artifacts for deployment")]
    Build(BuildArgs),
    #[command(about = "Run a migration, declaring and deploying contracts as necessary to update \
                       the world")]
    Migrate(Box<MigrateArgs>),
    #[command(about = "Execute a system with the given calldata.")]
    Execute(Box<ExecuteArgs>),
    #[command(about = "Inspect the world")]
    Inspect(Box<InspectArgs>),
    #[command(about = "Clean the build directory")]
    Clean(Box<CleanArgs>),
    #[command(about = "Call a contract")]
    Call(Box<CallArgs>),
    #[command(about = "Runs cairo tests")]
    Test(Box<TestArgs>),
    #[command(about = "Computes hash with different hash functions")]
    Hash(Box<HashArgs>),
    #[command(about = "Initialize a new dojo project")]
    Init(Box<InitArgs>),
}

impl fmt::Display for Commands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Commands::Build(_) => write!(f, "Build"),
            Commands::Clean(_) => write!(f, "Clean"),
            Commands::Execute(_) => write!(f, "Execute"),
            Commands::Inspect(_) => write!(f, "Inspect"),
            Commands::Migrate(_) => write!(f, "Migrate"),
            Commands::Call(_) => write!(f, "Call"),
            Commands::Test(_) => write!(f, "Test"),
            Commands::Hash(_) => write!(f, "Hash"),
            Commands::Init(_) => write!(f, "Init"),
        }
    }
}

pub fn run(command: Commands, config: &Config) -> Result<()> {
    let name = command.to_string();
    let span = info_span!("Subcommand", name);
    let _span = span.enter();

    // use `.map(|_| ())` to avoid returning a value here but still
    // useful to write tests for each command.

    match command {
        Commands::Build(args) => args.run(config),
        Commands::Migrate(args) => args.run(config),
        Commands::Execute(args) => args.run(config),
        Commands::Inspect(args) => args.run(config),
        Commands::Clean(args) => args.run(config),
        Commands::Call(args) => args.run(config),
        Commands::Test(args) => args.run(config),
        Commands::Hash(args) => args.run().map(|_| ()),
        Commands::Init(args) => args.run(config),
    }
}

/// Checks if the package has a compatible version of dojo-core.
/// In case of a workspace with multiple packages, each package is individually checked
/// and the workspace manifest path is returned in case of virtual workspace.
pub fn check_package_dojo_version(ws: &Workspace<'_>, package: &Package) -> anyhow::Result<()> {
    if let Some(dojo_dep) =
        package.manifest.summary.dependencies.iter().find(|dep| dep.name.as_str() == "dojo")
    {
        let dojo_version = env!("CARGO_PKG_VERSION");

        let dojo_dep_str = dojo_dep.to_string();

        // Only in case of git dependency with an explicit tag, we check if the tag is the same as
        // the current version.
        if dojo_dep_str.contains("git+")
            && dojo_dep_str.contains("tag=v")
            && !dojo_dep_str.contains(dojo_version)
        {
            if let Ok(cp) = ws.current_package() {
                let path =
                    if cp.id == package.id { package.manifest_path() } else { ws.manifest_path() };

                anyhow::bail!(
                    "Found dojo-core version mismatch: expected {}. Please verify your dojo \
                     dependency in {}",
                    dojo_version,
                    path
                )
            } else {
                // Virtual workspace.
                anyhow::bail!(
                    "Found dojo-core version mismatch: expected {}. Please verify your dojo \
                     dependency in {}",
                    dojo_version,
                    ws.manifest_path()
                )
            }
        }
    }

    Ok(())
}
