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
use scarb::core::Config;
use sozo_ops::account;
use starknet::signers::LocalWallet;
use starknet_crypto::FieldElement;

use super::options::fee::FeeOptions;
use super::options::signer::SignerOptions;
use super::options::starknet::StarknetOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct AccountArgs {
    #[clap(subcommand)]
    command: AccountCommand,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum AccountCommand {
    #[clap(about = "Create a new account configuration without actually deploying.")]
    New {
        #[clap(flatten)]
        signer: SignerOptions,

        #[clap(long, short, help = "Overwrite the account config file if it already exists")]
        force: bool,

        #[clap(help = "Path to save the account config file")]
        output: PathBuf,
    },

    #[clap(about = "Deploy account contract with a DeployAccount transaction.")]
    Deploy {
        #[clap(flatten)]
        starknet: StarknetOptions,

        #[clap(flatten)]
        signer: SignerOptions,

        #[clap(flatten)]
        fee: FeeOptions,

        #[clap(long, help = "Simulate the transaction only")]
        simulate: bool,

        #[clap(long, help = "Provide transaction nonce manually")]
        nonce: Option<FieldElement>,

        #[clap(
            long,
            env = "STARKNET_POLL_INTERVAL",
            default_value = "5000",
            help = "Transaction result poll interval in milliseconds"
        )]
        poll_interval: u64,

        #[clap(help = "Path to the account config file")]
        file: PathBuf,
    },
}

impl AccountArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let env_metadata = utils::load_metadata_from_config(config)?;

        config.tokio_handle().block_on(async {
            match self.command {
                AccountCommand::New { signer, force, output } => {
                    let signer: LocalWallet = signer.signer(env_metadata.as_ref()).unwrap();
                    account::new(signer, force, output).await
                }
                AccountCommand::Deploy {
                    starknet,
                    signer,
                    fee,
                    simulate,
                    nonce,
                    poll_interval,
                    file,
                } => {
                    let provider = starknet.provider(env_metadata.as_ref()).unwrap();
                    let signer = signer.signer(env_metadata.as_ref()).unwrap();
                    let fee_setting = fee.into_setting()?;
                    account::deploy(
                        provider,
                        signer,
                        fee_setting,
                        simulate,
                        nonce,
                        poll_interval,
                        file,
                    )
                    .await
                }
            }
        })
    }
}
