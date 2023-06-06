use std::env::{self, current_dir};

use camino::Utf8PathBuf;
use clap::Args;
use dojo_lang::compiler::DojoCompiler;
use dojo_lang::plugin::CairoPluginRepository;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;

use super::{ui_verbosity_from_flag, ProfileSpec};

#[derive(Args, Debug)]
pub struct BuildArgs {
    #[clap(help = "Source directory")]
    pub path: Option<Utf8PathBuf>,

    #[clap(help = "Specify the profile to use.")]
    #[command(flatten)]
    pub profile_spec: ProfileSpec,

    #[clap(help = "Logging verbosity.")]
    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
}

pub fn run(args: BuildArgs) -> anyhow::Result<()> {
    let source_dir = match args.path {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                let mut current_path = current_dir().unwrap();
                current_path.push(path);
                Utf8PathBuf::from_path_buf(current_path).unwrap()
            }
        }
        None => Utf8PathBuf::from_path_buf(current_dir().unwrap()).unwrap(),
    };

    let mut compilers = CompilerRepository::std();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let cairo_plugins = CairoPluginRepository::new();

    let manifest_path = source_dir.join("Scarb.toml");
    let config = Config::builder(manifest_path)
        .ui_verbosity(ui_verbosity_from_flag(args.verbose))
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .cairo_plugins(cairo_plugins.into())
        .profile(args.profile_spec.determine()?)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config)?;

    ops::compile(&ws)
}
