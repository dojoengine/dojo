use std::process::exit;

use clap::Parser;
use env_logger::Env;
use log::error;

mod build;
mod cli;
mod init;
mod migrate;

use cli::{App, Commands};

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let cli = App::parse();

    let res = match cli.command {
        Commands::Build(args) => build::run(args),
        Commands::Init(args) => {
            init::run(args);
            Ok(())
        }
        Commands::Migrate(args) => migrate::run(args),
        Commands::Bind(..) => Ok(print!("Bind")),
        Commands::Inspect(..) => Ok(print!("Inspect")),
    };

    if let Err(err) = res {
        error! {"{}", err};
        exit(1);
    }
}
