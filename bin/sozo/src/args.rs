use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use scarb::compiler::Profile;
use scarb_ui::Verbosity;
use smol_str::SmolStr;
use tracing::level_filters::LevelFilter;
use tracing_log::AsTrace;

use crate::commands::Commands;
use crate::utils::generate_version;

#[derive(Parser)]
#[command(author, version=generate_version(), about, long_about = None)]
#[command(propagate_version = true)]
pub struct SozoArgs {
    #[arg(long)]
    #[arg(global = true)]
    #[arg(hide_short_help = true)]
    #[arg(env = "DOJO_MANIFEST_PATH")]
    #[arg(help = "Override path to a directory containing a Scarb.toml file.")]
    pub manifest_path: Option<Utf8PathBuf>,

    #[clap(help = "Specify the profile to use.")]
    #[command(flatten)]
    pub profile_spec: ProfileSpec,

    #[clap(help = "Logging verbosity.")]
    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    #[arg(long)]
    #[arg(env = "SOZO_OFFLINE")]
    #[arg(hide_short_help = true)]
    #[arg(help = "Run without accessing the network.")]
    pub offline: bool,

    #[command(subcommand)]
    pub command: Commands,
}

impl SozoArgs {
    pub fn ui_verbosity(&self) -> Verbosity {
        let filter = self.verbose.log_level_filter().as_trace();
        if filter >= LevelFilter::WARN {
            Verbosity::Verbose
        } else if filter > LevelFilter::OFF {
            Verbosity::Normal
        } else {
            Verbosity::Quiet
        }
    }
}

/// Profile specifier.
#[derive(Parser, Clone, Debug)]
#[group(multiple = false)]
pub struct ProfileSpec {
    #[arg(short = 'P', long)]
    #[arg(help = "Specify profile to use by name.")]
    pub profile: Option<SmolStr>,

    #[arg(long, hide_short_help = true)]
    #[arg(help = "Use release profile.")]
    pub release: bool,

    #[arg(long, hide_short_help = true)]
    #[arg(help = "Use dev profile.")]
    pub dev: bool,
}

impl ProfileSpec {
    pub fn determine(&self) -> Result<Profile> {
        Ok(match &self {
            Self { release: true, .. } => Profile::RELEASE,
            Self { dev: true, .. } => Profile::DEV,
            Self { profile: Some(profile), .. } => Profile::new(profile.clone())?,
            _ => Profile::default(),
        })
    }
}
