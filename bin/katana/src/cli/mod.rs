use std::future::Future;

use anyhow::{Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use katana_cli::NodeArgs;
use katana_node::version::VERSION;
use tokio::runtime::Runtime;

mod config;
mod db;
mod init;

#[derive(Parser)]
#[command(name = "katana", author, version = VERSION, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    commands: Option<Commands>,

    #[command(flatten)]
    node: NodeArgs,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        if let Some(cmd) = self.commands {
            return match cmd {
                Commands::Db(args) => args.execute(),
                Commands::Config(args) => args.execute(),
                Commands::Completions(args) => args.execute(),
                Commands::Init(args) => execute_async(args.execute())?,
            };
        }

        execute_async(self.node.with_config_file()?.execute())?
    }
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Initialize chain")]
    Init(Box<init::InitArgs>),

    #[command(about = "Chain configuration utilities")]
    Config(config::ConfigArgs),

    #[command(about = "Database utilities")]
    Db(db::DbArgs),

    #[command(about = "Generate shell completion file for specified shell")]
    Completions(CompletionsArgs),
}

#[derive(Debug, Args)]
struct CompletionsArgs {
    shell: Shell,
}

impl CompletionsArgs {
    fn execute(self) -> Result<()> {
        let mut command = Cli::command();
        let name = command.get_name().to_string();
        clap_complete::generate(self.shell, &mut command, name, &mut std::io::stdout());
        Ok(())
    }
}

pub fn execute_async<F: Future>(future: F) -> Result<F::Output> {
    Ok(build_tokio_runtime().context("Failed to build tokio runtime")?.block_on(future))
}

fn build_tokio_runtime() -> std::io::Result<Runtime> {
    tokio::runtime::Builder::new_multi_thread().enable_all().build()
}
