use anyhow::Result;
use scarb::core::Config;

use crate::args::Commands;

pub(crate) mod build;
pub(crate) mod component;
pub(crate) mod events;
pub(crate) mod execute;
pub(crate) mod init;
pub(crate) mod migrate;
pub(crate) mod options;
pub(crate) mod register;
pub(crate) mod system;
pub(crate) mod test;

pub fn run(command: Commands, config: &Config) -> Result<()> {
    match command {
        Commands::Init(args) => args.run(config),
        Commands::Test(args) => args.run(config),
        Commands::Build(args) => args.run(config),
        Commands::Migrate(args) => args.run(config),

        Commands::Execute(args) => args.run(config),
        Commands::Component(args) => args.run(config),
        Commands::System(args) => args.run(config),
        Commands::Register(args) => args.run(config),
        Commands::Events(args) => args.run(config),
    }
}
