use std::env;
use std::process::exit;

use anyhow::Result;
use clap::Parser;
use dojo_lang::compiler::DojoCompiler;
use dojo_lang::plugin::CairoPluginRepository;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ui::{OutputFormat, Ui};

mod commands;
mod ops;

use commands::{App, Commands};

fn main() {
    let args = App::parse();

    let ui = Ui::new(args.ui_verbosity(), OutputFormat::Text);

    if let Err(err) = cli_main(args) {
        ui.anyhow(&err);
        exit(1);
    }
}

fn cli_main(args: App) -> Result<()> {
    let mut compilers = CompilerRepository::std();
    compilers.add(Box::new(DojoCompiler)).unwrap();
    let cairo_plugins = CairoPluginRepository::new();

    let manifest_path = scarb::ops::find_manifest_path(args.manifest_path.as_deref())?;

    let config = Config::builder(manifest_path)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .profile(args.profile_spec.determine()?)
        .cairo_plugins(cairo_plugins.into())
        .ui_verbosity(args.ui_verbosity())
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config)?;

    match args.command {
        Commands::Init(args) => args.run(),
        Commands::Test(args) => args.run(),
        Commands::Build(args) => args.run(&ws),
        Commands::Migrate(args) => args.run(&ws),
    }
}
