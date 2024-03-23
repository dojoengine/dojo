use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};
use sozo_ops::keystore;

#[derive(Debug, Args)]
pub struct KeystoreArgs {
    #[clap(subcommand)]
    command: KeystoreCommand,
}

#[derive(Debug, Subcommand)]
pub enum KeystoreCommand {
    #[clap(about = "Randomly generate a new keystore.")]
    New {
        #[clap(long, help = "Supply password from command line option instead of prompt")]
        password: Option<String>,

        #[clap(long, help = "Overwrite the file if it already exists")]
        force: bool,

        #[clap(help = "Path to save the JSON keystore")]
        file: PathBuf,
    },

    #[clap(about = "Create a keystore file from an existing private key.")]
    FromKey {
        #[clap(long, help = "Overwrite the file if it already exists")]
        force: bool,

        #[clap(long, help = "Supply private key from command line option instead of prompt")]
        private_key: Option<String>,

        #[clap(long, help = "Supply password from command line option instead of prompt")]
        password: Option<String>,

        #[clap(help = "Path to save the JSON keystore")]
        file: PathBuf,
    },

    #[clap(about = "Check the public key of an existing keystore file.")]
    Inspect {
        #[clap(long, help = "Supply password from command line option instead of prompt")]
        password: Option<String>,

        #[clap(long, help = "Print the public key only")]
        raw: bool,

        #[clap(help = "Path to the JSON keystore")]
        file: PathBuf,
    },

    #[clap(about = "Check the private key of an existing keystore file.")]
    InspectPrivate {
        #[clap(long, help = "Supply password from command line option instead of prompt")]
        password: Option<String>,

        #[clap(long, help = "Print the private key only")]
        raw: bool,

        #[clap(help = "Path to the JSON keystore")]
        file: PathBuf,
    },
}

impl KeystoreArgs {
    pub fn run(self) -> Result<()> {
        match self.command {
            KeystoreCommand::New { password, force, file } => keystore::new(password, force, file),
            KeystoreCommand::FromKey { force, private_key, password, file } => {
                keystore::from_key(force, private_key, password, file)
            }
            KeystoreCommand::Inspect { password, raw, file } => {
                keystore::inspect(password, raw, file)
            }
            KeystoreCommand::InspectPrivate { password, raw, file } => {
                keystore::inspect_private(password, raw, file)
            }
        }
    }
}
