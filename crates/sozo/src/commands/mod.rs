use anyhow::Result;
use scarb::core::Config;

use crate::args::Commands;

pub(crate) mod auth;
pub(crate) mod build;
pub(crate) mod completions;
pub(crate) mod component;
pub(crate) mod dev;
pub(crate) mod events;
pub(crate) mod execute;
pub(crate) mod init;
pub(crate) mod migrate;
pub(crate) mod options;
pub(crate) mod register;
pub(crate) mod test;

// copy of non pub functions from scarb
pub(crate) mod scarb_internal;

pub fn run(command: Commands, config: &Config) -> Result<()> {
    match command {
        Commands::Init(args) => args.run(config),
        Commands::Test(args) => args.run(config),
        Commands::Build(args) => args.run(config),
        Commands::Migrate(args) => args.run(config),
        Commands::Dev(args) => args.run(config),
        Commands::Auth(args) => args.run(config),
        Commands::Execute(args) => args.run(config),
        Commands::Component(args) => args.run(config),
        Commands::Register(args) => args.run(config),
        Commands::Events(args) => args.run(config),
        Commands::Completions(args) => args.run(),
    }
}
