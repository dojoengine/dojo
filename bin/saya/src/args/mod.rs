//! Saya binary options.
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use clap::Parser;
use dojo_utils::keystore::prompt_password_if_needed;
use saya_core::data_availability::celestia::CelestiaConfig;
use saya_core::data_availability::DataAvailabilityConfig;
use saya_core::{ProverAccessKey, SayaConfig, StarknetAccountData};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::signers::SigningKey;
use starknet_account::StarknetAccountOptions;
use tracing::Subscriber;
use tracing_subscriber::{fmt, EnvFilter};
use url::Url;

use crate::args::data_availability::{DataAvailabilityChain, DataAvailabilityOptions};
use crate::args::proof::ProofOptions;

mod data_availability;
mod proof;
mod starknet_account;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct SayaArgs {
    /// Specify the Katana URL to fetch data from.
    #[arg(long)]
    #[arg(value_name = "KATANA URL")]
    #[arg(help = "The Katana RPC URL to fetch data from.")]
    #[arg(default_value = "http://localhost:5050")]
    pub rpc_url: Url,

    #[arg(long)]
    #[arg(value_name = "STORE PROOFS")]
    #[arg(help = "When enabled all proofs are saved as a file.")]
    #[arg(default_value_t = false)]
    pub store_proofs: bool,

    /// Enable JSON logging.
    #[arg(long)]
    #[arg(help = "Output logs in JSON format.")]
    pub json_log: bool,

    /// Specify a JSON configuration file to use.
    #[arg(long)]
    #[arg(value_name = "CONFIG FILE")]
    #[arg(help = "The path to a JSON configuration file. This takes precedence over other CLI \
                  arguments.")]
    pub config_file: Option<PathBuf>,

    /// Specify a block to start fetching data from.
    #[arg(short, long, default_value = "0")]
    pub start_block: u64,

    #[arg(short, long, default_value = "1")]
    #[arg(help = "The number of blocks to be merged into a single proof.")]
    pub batch_size: usize,

    #[command(flatten)]
    #[command(next_help_heading = "Data availability options")]
    pub data_availability: DataAvailabilityOptions,

    #[command(flatten)]
    #[command(next_help_heading = "Choose the proof pipeline configuration")]
    pub proof: ProofOptions,

    #[command(flatten)]
    #[command(next_help_heading = "Starknet account configuration for settlement")]
    pub starknet_account: StarknetAccountOptions,
}

impl SayaArgs {
    pub fn init_logging(&self) -> Result<(), Box<dyn std::error::Error>> {
        const DEFAULT_LOG_FILTER: &str = "info,saya::core=trace,blockchain=trace,provider=trace";

        let builder = fmt::Subscriber::builder().with_env_filter(
            EnvFilter::try_from_default_env().or(EnvFilter::try_new(DEFAULT_LOG_FILTER))?,
        );

        let subscriber: Box<dyn Subscriber + Send + Sync> = if self.json_log {
            Box::new(builder.json().finish())
        } else {
            Box::new(builder.finish())
        };

        Ok(tracing::subscriber::set_global_default(subscriber)?)
    }
}

impl TryFrom<SayaArgs> for SayaConfig {
    type Error = Box<dyn std::error::Error>;

    fn try_from(args: SayaArgs) -> Result<Self, Self::Error> {
        let skip_publishing_proof = args.data_availability.celestia.skip_publishing_proof;

        if let Some(config_file) = args.config_file {
            let file = File::open(config_file).map_err(|_| "Failed to open config file")?;
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).map_err(|e| e.into())
        } else {
            let da_config = match args.data_availability.da_chain {
                Some(chain) => Some(match chain {
                    DataAvailabilityChain::Celestia => {
                        let conf = args.data_availability.celestia;

                        DataAvailabilityConfig::Celestia(CelestiaConfig {
                            node_url: match conf.celestia_node_url {
                                Some(v) => v,
                                None => {
                                    return Err(Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidInput,
                                        "Celestia config: Node url is required",
                                    )));
                                }
                            },
                            namespace: match conf.celestia_namespace {
                                Some(v) => v,
                                None => {
                                    return Err(Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidInput,
                                        "Celestia config: Namespace is required",
                                    )));
                                }
                            },
                            node_auth_token: conf.celestia_node_auth_token,
                        })
                    }
                }),
                None => None,
            };

            // Check if the private key is from keystore or provided directly to follow `sozo`
            // conventions.
            let private_key = if let Some(pk) = args.starknet_account.signer_key {
                pk
            } else if let Some(path) = args.starknet_account.signer_keystore_path {
                let password = prompt_password_if_needed(
                    args.starknet_account.signer_keystore_password.as_deref(),
                    false,
                )?;

                SigningKey::from_keystore(path, &password)?.secret_scalar()
            } else {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Could not find private key. Please specify the private key or path to the \
                     keystore file.",
                )));
            };

            let starknet_account = StarknetAccountData {
                starknet_url: args.starknet_account.starknet_url,
                chain_id: cairo_short_string_to_felt(&args.starknet_account.chain_id)?,
                signer_address: args.starknet_account.signer_address,
                signer_key: private_key,
            };

            let prover_key =
                ProverAccessKey::from_hex_string(&args.proof.private_key).map_err(|e| {
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))
                })?;

            Ok(SayaConfig {
                katana_rpc: args.rpc_url,
                prover_url: args.proof.prover_url,
                prover_key,
                store_proofs: args.store_proofs,
                start_block: args.start_block,
                batch_size: args.batch_size,
                data_availability: da_config,
                world_address: args.proof.world_address,
                fact_registry_address: args.proof.fact_registry_address,
                skip_publishing_proof,
                starknet_account,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use katana_primitives::felt::FieldElement;

    use super::*;
    use crate::args::data_availability::CelestiaOptions;

    #[test]
    fn test_saya_config_deserialization() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let config_file_path = std::path::Path::new(&manifest_dir)
            .join("src")
            .join("args")
            .join("test_saya_config_file.json");

        let args = SayaArgs {
            config_file: Some(config_file_path.clone()),
            rpc_url: Url::parse("http://localhost:5050").unwrap(),
            store_proofs: true,
            json_log: false,
            start_block: 0,
            batch_size: 4,
            data_availability: DataAvailabilityOptions {
                da_chain: None,
                celestia: CelestiaOptions {
                    celestia_node_url: None,
                    celestia_node_auth_token: None,
                    celestia_namespace: None,
                    skip_publishing_proof: true,
                },
            },
            proof: ProofOptions {
                world_address: Default::default(),
                fact_registry_address: Default::default(),
                prover_url: Url::parse("http://localhost:5050").unwrap(),
                private_key: Default::default(),
            },
            starknet_account: StarknetAccountOptions {
                starknet_url: Url::parse("http://localhost:5030").unwrap(),
                chain_id: "SN_SEPOLIA".to_string(),
                signer_address: Default::default(),
                signer_key: None,
                signer_keystore_path: None,
                signer_keystore_password: None,
            },
        };

        let config: SayaConfig = args.try_into().unwrap();

        assert_eq!(config.katana_rpc.as_str(), "http://localhost:5050/");
        assert_eq!(config.prover_url.as_str(), "http://localhost:1234/");
        assert_eq!(config.batch_size, 4);
        assert_eq!(
            config.prover_key.signing_key_as_hex_string(),
            "0xd0fa91f4949e9a777ebec071ca3ca6acc1f5cd6c6827f123b798f94e73425027"
        );
        assert!(!config.store_proofs);
        assert!(config.skip_publishing_proof);
        assert_eq!(config.start_block, 0);
        if let Some(DataAvailabilityConfig::Celestia(celestia_config)) = config.data_availability {
            assert_eq!(celestia_config.node_url.as_str(), "http://localhost:26657/");
            assert_eq!(celestia_config.node_auth_token, Some("your_auth_token".to_string()));
            assert_eq!(celestia_config.namespace, "katana");
        } else {
            panic!("Expected Celestia config");
        }

        let expected = StarknetAccountData {
            starknet_url: Url::parse("http://localhost:5030").unwrap(),
            chain_id: FieldElement::from_hex("0x534e5f5345504f4c4941").unwrap(),
            signer_address: FieldElement::from_hex(
                "0x3aa0a12c62a46a200b1a1211e8cd09b520164104e76d79648ca459cf05db94",
            )
            .unwrap(),
            signer_key: FieldElement::from_hex(
                "0x6b41bfa82e791a8b4e6b3ee058cb25b89714e4a23bd9a1ad6e6ba0bbc0b145b",
            )
            .unwrap(),
        };

        assert_eq!(config.starknet_account, expected);
    }
}
