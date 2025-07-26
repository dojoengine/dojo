use anyhow::{anyhow, Result};
use clap::{Args, ValueEnum};
use sozo_ops::migrate::{VerificationConfig, VoyagerConfig};
use url::Url;

#[derive(Debug, Clone, ValueEnum)]
pub enum VerificationService {
    /// Voyager mainnet verification service
    Voyager,
    /// Voyager Sepolia testnet verification service
    VoyagerSepolia,
    /// Voyager development verification service
    VoyagerDev,
    /// Custom verification service with user-provided URL
    Custom,
}

#[derive(Debug, Clone, Args)]
#[command(next_help_heading = "Contract verification options")]
pub struct ContractVerifyOption {
    /// Enable contract verification with specified service
    #[arg(
        long = "verify",
        value_enum,
        help = "Enable contract verification with specified service"
    )]
    pub service: Option<VerificationService>,

    /// Custom verification API URL (required when --verify=custom)
    #[arg(
        long = "verify-url",
        value_name = "URL",
        help = "Custom verification API URL (required when --verify=custom)"
    )]
    pub custom_url: Option<String>,

    /// Watch verification progress until completion
    #[arg(long = "verify-watch", help = "Watch verification progress until completion")]
    pub watch: bool,
}

#[derive(Debug, Clone, Args)]
#[command(next_help_heading = "Verification options")]
pub struct VerifyOptions {
    #[command(flatten)]
    pub contract: ContractVerifyOption,
}

impl VerifyOptions {
    /// Creates verification configuration based on the specified service
    pub fn create_verification_config(&self) -> Result<VerificationConfig> {
        self.contract.create_verification_config()
    }

    /// Builder method to create verification configuration
    pub fn build(&self) -> Result<VerificationConfig> {
        self.create_verification_config()
    }
}

impl ContractVerifyOption {
    /// Creates verification configuration based on the specified service
    pub fn create_verification_config(&self) -> Result<VerificationConfig> {
        if let Some(ref service) = self.service {
            let api_url = match service {
                VerificationService::Voyager => Url::parse("https://api.voyager.online/beta")?,
                VerificationService::VoyagerSepolia => {
                    Url::parse("https://sepolia-api.voyager.online/beta")?
                }
                VerificationService::VoyagerDev => {
                    Url::parse("https://dev-api.voyager.online/beta")?
                }
                VerificationService::Custom => {
                    if let Some(ref url) = self.custom_url {
                        Url::parse(url)?
                    } else {
                        return Err(anyhow!("--verify-url is required when using --verify=custom"));
                    }
                }
            };

            Ok(VerificationConfig::Voyager(VoyagerConfig::new(api_url, self.watch)))
        } else {
            Ok(VerificationConfig::None)
        }
    }
}
