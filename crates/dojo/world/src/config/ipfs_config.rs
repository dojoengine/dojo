use anyhow::Result;
use serde::Deserialize;

#[derive(Default, Deserialize, Clone, Debug)]
pub struct IpfsConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl IpfsConfig {
    pub fn assert_valid(&self) -> Result<()> {
        if self.url.is_empty() || self.username.is_empty() || self.password.is_empty() {
            anyhow::bail!("Invalid IPFS credentials: empty values not allowed");
        }
        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            anyhow::bail!("Invalid IPFS URL: must start with http:// or https://");
        }

        Ok(())
    }
}
