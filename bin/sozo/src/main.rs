#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::{env, sync::Arc};
use std::process::exit;

use anyhow::Result;
use args::SozoArgs;
use clap::Parser;
use scarb::compiler::plugin::CairoPluginRepository;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb_ui::{OutputFormat, Ui};
use tracing::trace;
mod args;
mod commands;
mod utils;

fn main() {
    let args = SozoArgs::parse();
    let _ = args.init_logging(&args.verbose);
    let ui = Ui::new(args.ui_verbosity(), OutputFormat::Text);

    if let Err(err) = cli_main(args) {
        ui.anyhow(&err);
        exit(1);
    }
}

fn cli_main(args: SozoArgs) -> Result<()> {
    let compilers = CompilerRepository::std();
    let cairo_plugins = CairoPluginRepository::std();

    let manifest_path = scarb::ops::find_manifest_path(args.manifest_path.as_deref())?;

    utils::verify_cairo_version_compatibility(&manifest_path)?;

    let config = Config::builder(manifest_path.clone())
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .profile(args.profile_spec.determine()?)
        .offline(args.offline)
        .cairo_plugins(cairo_plugins)
        .ui_verbosity(args.ui_verbosity())
        .compilers(compilers)
        .build()?;

    trace!(%manifest_path, "Configuration built successfully.");

    commands::run(args, &config)
}
