use anyhow::Result;
use clap::{Parser, Subcommand};
use scarb::compiler::Profile;
use scarb::ui;
use smol_str::SmolStr;
use tracing::level_filters::LevelFilter;
use tracing_log::AsTrace;

use self::build::BuildArgs;
use self::init::InitArgs;
use self::migrate::MigrateArgs;
use self::test::TestArgs;

pub(crate) mod build;
pub(crate) mod init;
pub(crate) mod migrate;
pub(crate) mod test;

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Build the world, generating the necessary artifacts for deployment")]
    Build(BuildArgs),
    #[command(about = "Initialize a new project")]
    Init(InitArgs),
    #[command(about = "Run a migration, declaring and deploying contracts as necessary to \
                       update the world")]
    Migrate(MigrateArgs),
    #[command(about = "Test the project's smart contracts")]
    Test(TestArgs),
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct App {
    #[command(subcommand)]
    pub command: Commands,
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

pub(crate) fn ui_verbosity_from_flag(verbose: clap_verbosity_flag::Verbosity) -> ui::Verbosity {
    let filter = verbose.log_level_filter().as_trace();
    if filter >= LevelFilter::WARN {
        ui::Verbosity::Verbose
    } else if filter > LevelFilter::OFF {
        ui::Verbosity::Normal
    } else {
        ui::Verbosity::Quiet
    }
}
