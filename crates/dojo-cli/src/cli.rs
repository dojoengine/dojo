mod build;
mod init;
mod migrate;
mod bind;

use build::BuildArgs;
use bind::BindArgs;
use clap::{Args, Parser, Subcommand};
use init::InitArgs;
use migrate::MigrateArgs;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(
        about = "Build the project's ECS, outputting smart contracts artifacts for deployment"
    )]
    Build(BuildArgs),
    #[command(about = "Initialize a new project")]
    Init(InitArgs),
    #[command(about = "Run a migration, declaring and deploying contracts as necessary to \
                       update the world")]
    Migrate(MigrateArgs),
    #[command(about = "Generate rust contract bindings")]
    Bind(BindArgs),
    #[command(about = "Retrieve an entity's state by entity ID")]
    Inspect(InspectArgs),
}



#[derive(Args)]
struct InspectArgs {
    #[clap(short, long, help = "Entity ID to retrieve state for")]
    id: String,
    #[clap(short, long, help = "World address to retrieve entity state from")]
    world_address: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build(args) => build::run(args),
        Commands::Init(args) => {
            init::run(args);
            Ok(())
        }
        Commands::Migrate(args) => migrate::run(args).await,
        Commands::Bind(args) => bind::run(args),
        Commands::Inspect(..) => Ok(print!("Inspect")),
    }?;
    Ok(())
}
