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
pub(crate) mod execute;
pub(crate) mod hash;
pub(crate) mod init;
pub(crate) mod inspect;
pub(crate) mod options;
pub(crate) mod test;

// TODO RBA
// pub(crate) mod dev;
// pub(crate) mod events;
// pub(crate) mod migrate;
// pub(crate) mod model;

use auth::AuthArgs;
use build::BuildArgs;
use call::CallArgs;
use clean::CleanArgs;
use execute::ExecuteArgs;
use hash::HashArgs;
use init::InitArgs;
use inspect::InspectArgs;
#[cfg(feature = "walnut")]
use sozo_walnut::walnut::WalnutArgs;
use test::TestArgs;

// TODO RBA
// use events::EventsArgs;
// use dev::DevArgs;
// use migrate::MigrateArgs;
// use model::ModelArgs;
//

pub(crate) const LOG_TARGET: &str = "sozo::cli";

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Grant or revoke a contract permission to write to a resource")]
    Auth(Box<AuthArgs>),
    #[command(about = "Build the world, generating the necessary artifacts for deployment")]
    Build(Box<BuildArgs>),
    #[command(about = "Call a contract")]
    Call(Box<CallArgs>),
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
    #[command(about = "Runs cairo tests")]
    Test(Box<TestArgs>),
    #[cfg(feature = "walnut")]
    #[command(about = "Interact with walnut.dev - transactions debugger and simulator")]
    Walnut(Box<WalnutArgs>),
    // TODO RBA
    // #[command(about = "Build and migrate the world every time a file changes")]
    // Dev(Box<DevArgs>),
    // #[command(about = "Run a migration, declaring and deploying contracts as necessary to
    // update \ the world")]
    // Migrate(Box<MigrateArgs>),

    //

    // #[command(about = "Inspect a model")]
    // Model(Box<ModelArgs>),
    // #[command(about = "Inspect events emitted by the world")]
    // Events(Box<EventsArgs>),
    //
}

impl fmt::Display for Commands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Commands::Auth(_) => write!(f, "Auth"),
            Commands::Build(_) => write!(f, "Build"),
            Commands::Call(_) => write!(f, "Call"),
            Commands::Clean(_) => write!(f, "Clean"),
            Commands::Execute(_) => write!(f, "Execute"),
            Commands::Hash(_) => write!(f, "Hash"),
            Commands::Init(_) => write!(f, "Init"),
            Commands::Inspect(_) => write!(f, "Inspect"),
            Commands::Test(_) => write!(f, "Test"),
            #[cfg(feature = "walnut")]
            Commands::Walnut(_) => write!(f, "WalnutVerify"),
            // Commands::Dev(_) => write!(f, "Dev"),
            // Commands::Migrate(_) => write!(f, "Migrate"),
            //
            // Commands::Model(_) => write!(f, "Model"),
            // Commands::Events(_) => write!(f, "Events"),
            //
        }
    }
}

pub async fn run(command: Commands, scarb_metadata: &Metadata, ui: &Ui) -> Result<()> {
    let name = command.to_string();
    let span = info_span!("Subcommand", name);
    let _span = span.enter();

    // use `.map(|_| ())` to avoid returning a value here but still
    // useful to write tests for each command.

    match command {
        Commands::Auth(args) => args.run(scarb_metadata),
        Commands::Build(args) => args.run(scarb_metadata).await,
        Commands::Call(args) => args.run(scarb_metadata),
        Commands::Clean(args) => args.run(scarb_metadata),
        Commands::Execute(args) => args.run(scarb_metadata),
        Commands::Hash(args) => args.run(scarb_metadata).map(|_| ()),
        Commands::Init(args) => args.run(scarb_metadata, ui),
        Commands::Inspect(args) => args.run(scarb_metadata),
        Commands::Test(args) => args.run(scarb_metadata),
        #[cfg(feature = "walnut")]
        Commands::Walnut(args) => args.run(scarb_metadata, ui),
        // TODO RBA
        // Commands::Dev(args) => args.run(config),
        // Commands::Migrate(args) => args.run(config),
        //
        // Commands::Model(args) => args.run(config),
        // Commands::Events(args) => args.run(config),
    }
}
