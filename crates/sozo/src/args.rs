use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use scarb::compiler::Profile;
use scarb::ui;
use smol_str::SmolStr;
use tracing::level_filters::LevelFilter;
use tracing_log::AsTrace;

use crate::commands::build::BuildArgs;
use crate::commands::component::ComponentArgs;
use crate::commands::events::EventsArgs;
use crate::commands::execute::ExecuteArgs;
use crate::commands::init::InitArgs;
use crate::commands::migrate::MigrateArgs;
use crate::commands::register::RegisterArgs;
use crate::commands::system::SystemArgs;
use crate::commands::test::TestArgs;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
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

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Build the world, generating the necessary artifacts for deployment")]
    Build(BuildArgs),
    #[command(about = "Initialize a new project")]
    Init(InitArgs),
    #[command(about = "Run a migration, declaring and deploying contracts as necessary to \
                       update the world")]
    Migrate(Box<MigrateArgs>),
    #[command(about = "Test the project's smart contracts")]
    Test(TestArgs),
    #[command(about = "Execute a world's system")]
    Execute(ExecuteArgs),
    #[command(about = "Interact with a worlds components")]
    Component(ComponentArgs),
    #[command(about = "Interact with a worlds systems")]
    System(SystemArgs),
    #[command(about = "Register new systems and components")]
    Register(RegisterArgs),
    #[command(about = "Queries world events")]
    Events(EventsArgs),
}

impl SozoArgs {
    pub fn ui_verbosity(&self) -> ui::Verbosity {
        let filter = self.verbose.log_level_filter().as_trace();
        if filter >= LevelFilter::WARN {
            ui::Verbosity::Verbose
        } else if filter > LevelFilter::OFF {
            ui::Verbosity::Normal
        } else {
            ui::Verbosity::Quiet
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
