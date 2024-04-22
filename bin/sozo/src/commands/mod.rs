use anyhow::Result;
use clap::{command, Subcommand};
use scarb::core::Config;

pub(crate) mod account;
pub(crate) mod auth;
pub(crate) mod build;
pub(crate) mod call;
pub(crate) mod clean;
pub(crate) mod completions;
pub(crate) mod dev;
pub(crate) mod events;
pub(crate) mod execute;
pub(crate) mod init;
pub(crate) mod keystore;
pub(crate) mod migrate;
pub(crate) mod model;
pub(crate) mod options;
pub(crate) mod register;
pub(crate) mod test;

use account::AccountArgs;
use auth::AuthArgs;
use build::BuildArgs;
use call::CallArgs;
use clean::CleanArgs;
use completions::CompletionsArgs;
use dev::DevArgs;
use events::EventsArgs;
use execute::ExecuteArgs;
use init::InitArgs;
use keystore::KeystoreArgs;
use migrate::MigrateArgs;
use model::ModelArgs;
use register::RegisterArgs;
use test::TestArgs;

#[derive(Subcommand)]
#[derive(Debug)]
pub enum Commands {
    #[command(about = "Manage accounts")]
    Account(AccountArgs),
    #[command(about = "Manage keystore files")]
    Keystore(KeystoreArgs),
    #[command(about = "Build the world, generating the necessary artifacts for deployment")]
    Build(BuildArgs),
    #[command(about = "Initialize a new project")]
    Init(InitArgs),
    #[command(about = "Remove generated artifacts, manifests and abis")]
    Clean(CleanArgs),
    #[command(about = "Run a migration, declaring and deploying contracts as necessary to \
                       update the world")]
    Migrate(Box<MigrateArgs>),
    #[command(about = "Developer mode: watcher for building and migration")]
    Dev(DevArgs),
    #[command(about = "Test the project's smart contracts")]
    Test(TestArgs),
    #[command(about = "Execute a world's system")]
    Execute(ExecuteArgs),
    #[command(about = "Call a world's system")]
    Call(CallArgs),
    #[command(about = "Interact with a worlds models")]
    Model(ModelArgs),
    #[command(about = "Register new models")]
    Register(RegisterArgs),
    #[command(about = "Queries world events")]
    Events(EventsArgs),
    #[command(about = "Manage world authorization")]
    Auth(AuthArgs),
    #[command(about = "Generate shell completion file for specified shell")]
    Completions(CompletionsArgs),
}

pub fn run(command: Commands, config: &Config) -> Result<()> {
    match command {
        Commands::Account(args) => args.run(config),
        Commands::Keystore(args) => args.run(config),
        Commands::Init(args) => args.run(config),
        Commands::Clean(args) => args.run(config),
        Commands::Test(args) => args.run(config),
        Commands::Build(args) => args.run(config),
        Commands::Migrate(args) => args.run(config),
        Commands::Dev(args) => args.run(config),
        Commands::Auth(args) => args.run(config),
        Commands::Execute(args) => args.run(config),
        Commands::Call(args) => args.run(config),
        Commands::Model(args) => args.run(config),
        Commands::Register(args) => args.run(config),
        Commands::Events(args) => args.run(config),
        Commands::Completions(args) => args.run(),
    }
}
