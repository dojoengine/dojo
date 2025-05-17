use core::fmt;

use anyhow::Result;
use auth::AuthArgs;
use clap::Subcommand;
// use events::EventsArgs;
use scarb_metadata::Metadata;
use tracing::info_span;

pub(crate) mod auth;
pub(crate) mod build;
pub(crate) mod options;
pub(crate) mod test;

// TODO RBA
// pub(crate) mod call;
// pub(crate) mod clean;
// pub(crate) mod dev;
// pub(crate) mod events;
// pub(crate) mod execute;
// pub(crate) mod hash;
// pub(crate) mod init;
// pub(crate) mod inspect;
// pub(crate) mod migrate;
// pub(crate) mod model;

use build::BuildArgs;
use test::TestArgs;

// TODO RBA
// use call::CallArgs;
// use clean::CleanArgs;
// use dev::DevArgs;
// use execute::ExecuteArgs;
// use hash::HashArgs;
// use init::InitArgs;
// use inspect::InspectArgs;
// use migrate::MigrateArgs;
// use model::ModelArgs;
// #[cfg(feature = "walnut")]
// use sozo_walnut::walnut::WalnutArgs;
//

pub(crate) const LOG_TARGET: &str = "sozo::cli";

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Grant or revoke a contract permission to write to a resource")]
    Auth(Box<AuthArgs>),
    #[command(about = "Build the world, generating the necessary artifacts for deployment")]
    Build(Box<BuildArgs>),
    #[command(about = "Runs cairo tests")]
    Test(Box<TestArgs>),
    // TODO RBA
    // #[command(about = "Build and migrate the world every time a file changes")]
    // Dev(Box<DevArgs>),
    // #[command(about = "Run a migration, declaring and deploying contracts as necessary to
    // update \ the world")]
    // Migrate(Box<MigrateArgs>),
    // #[command(about = "Execute one or several systems with the given calldata.")]
    // Execute(Box<ExecuteArgs>),
    // #[command(about = "Inspect the world")]
    // Inspect(Box<InspectArgs>),
    // #[command(about = "Clean the build directory")]
    // Clean(Box<CleanArgs>),
    // #[command(about = "Call a contract")]
    // Call(Box<CallArgs>),
    //
    // #[command(about = "Computes hash with different hash functions")]
    // Hash(Box<HashArgs>),
    // #[command(about = "Initialize a new dojo project")]
    // Init(Box<InitArgs>),
    // #[command(about = "Inspect a model")]
    // Model(Box<ModelArgs>),
    // #[command(about = "Inspect events emitted by the world")]
    // Events(Box<EventsArgs>),
    // #[cfg(feature = "walnut")]
    // #[command(about = "Interact with walnut.dev - transactions debugger and simulator")]
    // Walnut(Box<WalnutArgs>),
}

impl fmt::Display for Commands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Commands::Auth(_) => write!(f, "Auth"),
            Commands::Build(_) => write!(f, "Build"),
            Commands::Test(_) => write!(f, "Test"),
            // Commands::Clean(_) => write!(f, "Clean"),
            // Commands::Dev(_) => write!(f, "Dev"),
            // Commands::Execute(_) => write!(f, "Execute"),
            // Commands::Inspect(_) => write!(f, "Inspect"),
            // Commands::Migrate(_) => write!(f, "Migrate"),
            // Commands::Call(_) => write!(f, "Call"),
            //
            // Commands::Hash(_) => write!(f, "Hash"),
            // Commands::Init(_) => write!(f, "Init"),
            // Commands::Model(_) => write!(f, "Model"),
            // Commands::Events(_) => write!(f, "Events"),
            // #[cfg(feature = "walnut")]
            // Commands::Walnut(_) => write!(f, "WalnutVerify"),
        }
    }
}

pub async fn run(command: Commands, scarb_metadata: &Metadata) -> Result<()> {
    let name = command.to_string();
    let span = info_span!("Subcommand", name);
    let _span = span.enter();

    // use `.map(|_| ())` to avoid returning a value here but still
    // useful to write tests for each command.

    match command {
        Commands::Auth(args) => args.run(scarb_metadata),
        Commands::Build(args) => args.run(scarb_metadata).await,
        Commands::Test(args) => args.run(scarb_metadata),
        // TODO RBA
        // Commands::Dev(args) => args.run(config),
        // Commands::Migrate(args) => args.run(config),
        // Commands::Execute(args) => args.run(config),
        // Commands::Inspect(args) => args.run(config),
        // Commands::Clean(args) => args.run(config),
        // Commands::Call(args) => args.run(config),
        //
        // Commands::Hash(args) => args.run(config).map(|_| ()),
        // Commands::Init(args) => args.run(config),
        // Commands::Model(args) => args.run(config),
        // Commands::Events(args) => args.run(config),
        // #[cfg(feature = "walnut")]
        // Commands::Walnut(args) => args.run(config),
    }
}
