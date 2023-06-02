use std::process::exit;

use clap::Parser;
use env_logger::Env;
use log::error;

mod commands;

use self::commands::{build, init, migrate, App, Commands};

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("sozo=info")).init();

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
        Commands::Bind(..) => Ok(print!("Bind")),
        Commands::Inspect(..) => Ok(print!("Inspect")),
    };

    if let Err(err) = res {
        error! {"{}", err};
        exit(1);
    }
}
