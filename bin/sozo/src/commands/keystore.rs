// MIT License

// Copyright (c) 2022 Jonathan LEI

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

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

        #[clap(long, help = "Take the private key from stdin instead of prompt")]
        private_key_stdin: bool,

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
            KeystoreCommand::FromKey { force, private_key_stdin, password, file } => {
                keystore::from_key(force, private_key_stdin, password, file)
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
