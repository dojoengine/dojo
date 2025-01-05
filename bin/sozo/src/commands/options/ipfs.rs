use clap::Args;
use dojo_utils::env::{IPFS_PASSWORD_ENV_VAR, IPFS_URL_ENV_VAR, IPFS_USERNAME_ENV_VAR};
use dojo_world::config::IpfsConfig;
use tracing::trace;
use url::Url;

#[derive(Debug, Default, Args, Clone)]
#[command(next_help_heading = "IPFS options")]
pub struct IpfsOptions {
    #[arg(long, env = IPFS_URL_ENV_VAR)]
    #[arg(value_name = "URL")]
    #[arg(help = "The IPFS URL.")]
    #[arg(global = true)]
    pub ipfs_url: Option<Url>,

    #[arg(long, env = IPFS_USERNAME_ENV_VAR)]
    #[arg(value_name = "USERNAME")]
    #[arg(help = "The IPFS username.")]
    #[arg(global = true)]
    pub ipfs_username: Option<String>,

    #[arg(long, env = IPFS_PASSWORD_ENV_VAR)]
    #[arg(value_name = "PASSWORD")]
    #[arg(help = "The IPFS password.")]
    #[arg(global = true)]
    pub ipfs_password: Option<String>,
}

impl IpfsOptions {
    pub fn config(&self) -> Option<IpfsConfig> {
        trace!("Retrieving IPFS config for IpfsOptions.");

        let url = self.ipfs_url.as_ref().map(|url| url.to_string());
        let username = self.ipfs_username.clone();
        let password = self.ipfs_password.clone();

        if let (Some(url), Some(username), Some(password)) = (url, username, password) {
            Some(IpfsConfig { url, username, password })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use dojo_utils::env::{IPFS_PASSWORD_ENV_VAR, IPFS_URL_ENV_VAR, IPFS_USERNAME_ENV_VAR};

    use super::IpfsOptions;

    #[derive(clap::Parser)]
    struct Command {
        #[clap(flatten)]
        options: IpfsOptions,
    }

    const ENV_IPFS_URL: &str = "http://ipfs.service/";
    const ENV_IPFS_USERNAME: &str = "johndoe";
    const ENV_IPFS_PASSWORD: &str = "123456";

    #[test]
    fn options_read_from_env_variable() {
        std::env::set_var(IPFS_URL_ENV_VAR, ENV_IPFS_URL);
        std::env::set_var(IPFS_USERNAME_ENV_VAR, ENV_IPFS_USERNAME);
        std::env::set_var(IPFS_PASSWORD_ENV_VAR, ENV_IPFS_PASSWORD);

        let cmd = Command::parse_from([""]);
        let config = cmd.options.config().unwrap();
        assert_eq!(config.url, ENV_IPFS_URL.to_string());
        assert_eq!(config.username, ENV_IPFS_USERNAME.to_string());
        assert_eq!(config.password, ENV_IPFS_PASSWORD.to_string());
    }

    #[test]
    fn cli_args_override_env_variables() {
        std::env::set_var(IPFS_URL_ENV_VAR, ENV_IPFS_URL);
        let url = "http://different.url/";
        let username = "bobsmith";
        let password = "654321";

        let cmd = Command::parse_from([
            "sozo",
            "--ipfs-url",
            url,
            "--ipfs-username",
            username,
            "--ipfs-password",
            password,
        ]);
        let config = cmd.options.config().unwrap();
        assert_eq!(config.url, url);
        assert_eq!(config.username, username);
        assert_eq!(config.password, password);
    }

    #[test]
    fn invalid_url_format() {
        let cmd = Command::try_parse_from([
            "sozo",
            "--ipfs-url",
            "invalid-url",
            "--ipfs-username",
            "bobsmith",
            "--ipfs-password",
            "654321",
        ]);
        assert!(cmd.is_err());
    }

    #[test]
    fn options_not_provided_in_env_variable() {
        let cmd = Command::parse_from(["sozo"]);
        assert!(cmd.options.config().is_none());
    }
}
