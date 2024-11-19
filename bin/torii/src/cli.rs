//! CLI for Torii.
//!
//! Use a `Cli` struct to parse the CLI arguments
//! and to have flexibility in the future to add more commands
//! that may not start Torii directly.
use clap::Parser;
use torii_cli::ToriiArgs;

#[derive(Parser)]
#[command(name = "torii", author, version, about, long_about = None)]
pub struct Cli {
    #[command(flatten)]
    pub args: ToriiArgs,
}
