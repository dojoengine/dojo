use anyhow::{Result, anyhow};
use clap::Args;
use sozo_ops::migrate::VerificationConfig;
use url::Url;

#[derive(Debug, Clone, Args)]
#[command(next_help_heading = "Verification options")]
pub struct VerifyOptions {
    /// Enable contract verification with specified service
    /// Supported services: voyager, voyager-sepolia, voyager-dev, custom
    #[arg(long, value_name = "SERVICE")]
    pub verify: Option<String>,

    /// Custom verification API URL (used when --verify=custom)
    #[arg(long, value_name = "URL")]
    pub verify_url: Option<String>,

    /// Watch verification progress until completion
    #[arg(long, default_value_t = false)]
    pub verify_watch: bool,
}

impl VerifyOptions {
    /// Creates verification configuration based on the specified service
    pub fn create_verification_config(&self) -> Result<Option<VerificationConfig>> {
        if let Some(ref service) = self.verify {
            let api_url = match service.to_lowercase().as_str() {
                "voyager" => Url::parse("https://api.voyager.online/beta")?,
                "voyager-sepolia" => Url::parse("https://sepolia-api.voyager.online/beta")?,
                "voyager-dev" => Url::parse("https://dev-api.voyager.online/beta")?,
                "custom" => {
                    if let Some(ref url) = self.verify_url {
                        Url::parse(url)?
                    } else {
                        return Err(anyhow!("--verify-url is required when using --verify=custom"));
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "Unsupported verification service: {}. Supported services: voyager, \
                         voyager-sepolia, voyager-dev, custom",
                        service
                    ));
                }
            };

            Ok(Some(VerificationConfig {
                api_url,
                watch: self.verify_watch,
                include_tests: true, // Default to including tests for Dojo projects
                timeout: 300,        // 5 minutes default timeout
            }))
        } else {
            Ok(None)
        }
    }
}
