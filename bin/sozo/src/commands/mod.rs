use core::fmt;

use anyhow::Result;
use clap::Subcommand;
use scarb_metadata::Metadata;
use scarb_ui::Ui;
use tracing::info_span;

pub(crate) mod auth;
pub(crate) mod build;
pub(crate) mod call;
pub(crate) mod clean;
pub(crate) mod events;
pub(crate) mod execute;
pub(crate) mod hash;
pub(crate) mod init;
pub(crate) mod inspect;
pub(crate) mod migrate;
pub(crate) mod model;
pub(crate) mod options;
pub(crate) mod test;
pub(crate) mod version;

use auth::AuthArgs;
use build::BuildArgs;
use call::CallArgs;
use clean::CleanArgs;
use events::EventsArgs;
use execute::ExecuteArgs;
use hash::HashArgs;
use init::InitArgs;
use inspect::InspectArgs;
use migrate::MigrateArgs;
use model::ModelArgs;
#[cfg(feature = "walnut")]
use sozo_walnut::walnut::WalnutArgs;
use test::TestArgs;
use version::VersionArgs;

pub(crate) const LOG_TARGET: &str = "sozo::cli";

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Grant or revoke a contract permission to write to a resource")]
    Auth(Box<AuthArgs>),
    #[command(about = "Build the world, generating the necessary artifacts for deployment")]
    Build(Box<BuildArgs>),
    #[command(about = "Call a contract")]
    Call(Box<CallArgs>),
    #[command(about = "Inspect events emitted by the world")]
    Events(Box<EventsArgs>),
    #[command(about = "Execute one or several systems with the given calldata.")]
    Execute(Box<ExecuteArgs>),
    #[command(about = "Clean the build directory")]
    Clean(Box<CleanArgs>),
    #[command(about = "Computes hash with different hash functions")]
    Hash(Box<HashArgs>),
    #[command(about = "Initialize a new dojo project")]
    Init(Box<InitArgs>),
    #[command(about = "Inspect the world")]
    Inspect(Box<InspectArgs>),
    #[command(
        about = "Run a migration, declaring and deploying contracts as necessary to update the world"
    )]
    Migrate(Box<MigrateArgs>),
    #[command(about = "Inspect a model")]
    Model(Box<ModelArgs>),
    #[command(about = "Runs cairo tests")]
    Test(Box<TestArgs>),
    #[command(about = "Print version")]
    Version(Box<VersionArgs>),
    #[cfg(feature = "walnut")]
    #[command(about = "Interact with walnut.dev - transactions debugger and simulator")]
    Walnut(Box<WalnutArgs>),
}

impl fmt::Display for Commands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Commands::Auth(_) => write!(f, "Auth"),
            Commands::Build(_) => write!(f, "Build"),
            Commands::Call(_) => write!(f, "Call"),
            Commands::Clean(_) => write!(f, "Clean"),
            Commands::Events(_) => write!(f, "Events"),
            Commands::Execute(_) => write!(f, "Execute"),
            Commands::Hash(_) => write!(f, "Hash"),
            Commands::Init(_) => write!(f, "Init"),
            Commands::Inspect(_) => write!(f, "Inspect"),
            Commands::Migrate(_) => write!(f, "Migrate"),
            Commands::Model(_) => write!(f, "Model"),
            Commands::Test(_) => write!(f, "Test"),
            Commands::Version(_) => write!(f, "Version"),
            #[cfg(feature = "walnut")]
            Commands::Walnut(_) => write!(f, "WalnutVerify"),
        }
    }
}

pub async fn run(command: Commands, scarb_metadata: &Metadata, ui: &Ui) -> Result<()> {
    let name = command.to_string();
    let span = info_span!("Subcommand", name);
    let _span = span.enter();

    match command {
        Commands::Auth(args) => args.run(scarb_metadata).await,
        Commands::Build(args) => args.run(scarb_metadata).await,
        Commands::Call(args) => args.run(scarb_metadata).await,
        Commands::Clean(args) => args.run(scarb_metadata),
        Commands::Events(args) => args.run(scarb_metadata).await,
        Commands::Execute(args) => args.run(scarb_metadata, ui).await,
        Commands::Hash(args) => args.run(scarb_metadata),
        Commands::Inspect(args) => args.run(scarb_metadata).await,
        Commands::Migrate(args) => args.run(scarb_metadata).await,
        Commands::Model(args) => args.run(scarb_metadata).await,
        Commands::Test(args) => args.run(scarb_metadata),
        Commands::Version(args) => args.run(scarb_metadata),
        #[cfg(feature = "walnut")]
        Commands::Walnut(args) => args.run(scarb_metadata, ui).await,
        Commands::Init(_) => {
            // `sozo init` is directly managed in main.rs as scarb metadata
            // cannot be loaded in this case (the project does not exist yet).
            Ok(())
        }
    }
}
