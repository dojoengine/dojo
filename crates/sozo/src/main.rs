use std::process::exit;

use clap::Parser;
use scarb::ui::{OutputFormat, Ui, Verbosity};

mod commands;
mod ops;

use self::commands::{build, init, migrate, test, App, Commands};

fn main() {
    let cli = App::parse();

    let res = match cli.command {
        Commands::Build(args) => build::run(args),
        Commands::Init(args) => {
            match init::run(args) {
                Ok(_) => (),
                Err(e) => eprintln!("Error: {}", e),
            };
            Ok(())
        }
        Commands::Migrate(args) => migrate::run(args),
        Commands::Test(args) => test::run(args),
    };

    if let Err(err) = res {
        Ui::new(Verbosity::Normal, OutputFormat::Text).anyhow(&err);
        exit(1);
    }
}
