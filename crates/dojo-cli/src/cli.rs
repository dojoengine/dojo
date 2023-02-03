mod build;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Build the project's ECS, outputting smart contracts for deployment")]
    Build(BuildArgs),
    #[command(about = "Run a migration, declaring and deploying contracts as necessary to \
                       update the world")]
    Migrate(MigrateArgs),
    #[command(about = "Generate rust contract bindings")]
    Bind(BindArgs),
    #[command(about = "Retrieve an entity's state by entity ID")]
    Inspect(InspectArgs),
}

#[derive(Args)]
struct BuildArgs {}

#[derive(Args)]
struct MigrateArgs {
    #[clap(short, long, help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,
    #[clap(short, long, help = "World address to run migration on")]
    world_address: String,
}

#[derive(Args)]
struct BindArgs {}

#[derive(Args)]
struct InspectArgs {
    #[clap(short, long, help = "Entity ID to retrieve state for")]
    id: String,
    #[clap(short, long, help = "World address to retrieve entity state from")]
    world_address: String,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build(..) => print!("Build"),
        Commands::Migrate(..) => print!("Migrate"),
        Commands::Bind(..) => print!("Bind"),
        Commands::Inspect(..) => print!("Inspect"),
    }
    Ok(())
}
