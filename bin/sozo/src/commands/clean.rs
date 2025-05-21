use anyhow::Result;
use clap::Args;
use scarb_interop::MetadataDojoExt;
use scarb_metadata::Metadata;

#[derive(Debug, Args)]
pub struct CleanArgs {
    #[arg(long)]
    #[arg(help = "Clean all profiles.")]
    pub all_profiles: bool,
}

impl CleanArgs {
    pub fn run(self, scarb_metadata: &Metadata) -> Result<()> {
        if self.all_profiles {
            scarb_metadata.clean_dir_all_profiles();
        } else {
            scarb_metadata.clean_dir_profile();
        }

        Ok(())
    }
}
