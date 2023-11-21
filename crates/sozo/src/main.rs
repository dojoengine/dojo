use std::env;
use std::process::exit;
use std::str::FromStr;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use dojo_lang::compiler::DojoCompiler;
use dojo_lang::plugin::CairoPluginRepository;
use scarb::compiler::CompilerRepository;
use scarb::core::{Config, TomlManifest};
use scarb_ui::{OutputFormat, Ui};
use semver::Version;
use sozo::args::{Commands, SozoArgs};

fn main() {
    let args = SozoArgs::parse();

    let ui = Ui::new(args.ui_verbosity(), OutputFormat::Text);

    if let Err(err) = cli_main(args) {
        ui.anyhow(&err);
        exit(1);
    }
}

fn cli_main(args: SozoArgs) -> Result<()> {
    let mut compilers = CompilerRepository::std();
    let cairo_plugins = CairoPluginRepository::default();

    match &args.command {
        Commands::Build(_) | Commands::Dev(_) => compilers.add(Box::new(DojoCompiler)).unwrap(),
        _ => {}
    }

    let manifest_path = scarb::ops::find_manifest_path(args.manifest_path.as_deref())?;

    verify_cairo_version_compatibility(&manifest_path)?;

    let config = Config::builder(manifest_path)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .profile(args.profile_spec.determine()?)
        .offline(args.offline)
        .cairo_plugins(cairo_plugins.into())
        .ui_verbosity(args.ui_verbosity())
        .compilers(compilers)
        .build()?;

    sozo::commands::run(args.command, &config)
}

fn verify_cairo_version_compatibility(manifest_path: &Utf8PathBuf) -> Result<()> {
    let scarb_cairo_version = scarb::version::get().cairo;
    // When manifest file doesn't exists ignore it. Would be the case during `sozo init`
    let Ok(manifest) = TomlManifest::read_from_path(manifest_path) else { return Ok(()) };

    // For any kind of error, like package not specified, cairo version not specified return
    // without an error
    let Some(package) = manifest.package else { return Ok(()) };

    let Some(cairo_version) = package.cairo_version else { return Ok(()) };

    // only when cairo version is found in manifest file confirm that it matches
    let version_req = cairo_version.as_defined().unwrap();
    let version = Version::from_str(scarb_cairo_version.version).unwrap();
    if !version_req.matches(&version) {
        anyhow::bail!(
            "Specified cairo version not supported by dojo. Please verify and update dojo."
        );
    };

    Ok(())
}
