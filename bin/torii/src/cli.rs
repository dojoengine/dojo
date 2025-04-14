//! CLI for Torii.
//!
//! Use a `Cli` struct to parse the CLI arguments
//! and to have flexibility in the future to add more commands
//! that may not start Torii directly.
use clap::Parser;
use torii_cli::ToriiArgs;

#[derive(Parser)]
#[command(name = "torii", author, about, long_about = None, version = env!("TORII_VERSION_SPEC"))]
#[command(next_help_heading = "Torii general options")]
pub struct Cli {
    #[command(flatten)]
    pub args: ToriiArgs,
}
