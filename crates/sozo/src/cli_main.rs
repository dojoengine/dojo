use std::env;

use anyhow::Result;
use dojo_lang::compiler::DojoCompiler;
use dojo_lang::plugin::CairoPluginRepository;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;

use crate::args::{Commands, SozoArgs};

pub fn cli_main(args: SozoArgs) -> Result<()> {
    let mut compilers = CompilerRepository::std();
    let cairo_plugins = CairoPluginRepository::new();

    match &args.command {
        Commands::Build(_) | Commands::Dev(_) => compilers.add(Box::new(DojoCompiler)).unwrap(),
        _ => {}
    }

    let manifest_path = scarb::ops::find_manifest_path(args.manifest_path.as_deref())?;

    let config = Config::builder(manifest_path)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .profile(args.profile_spec.determine()?)
        .offline(args.offline)
        .cairo_plugins(cairo_plugins.into())
        .ui_verbosity(args.ui_verbosity())
        .compilers(compilers)
        .build()?;

    crate::commands::run(args.command, &config)
}
