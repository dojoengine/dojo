use clap::Args;
use dojo_utils::env::{IPFS_PASSWORD_ENV_VAR, IPFS_URL_ENV_VAR, IPFS_USERNAME_ENV_VAR};
use dojo_world::config::Environment;
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
    pub fn url(&self, env_metadata: Option<&Environment>) -> Option<String> {
        trace!("Retrieving URL for IpfsOptions.");

        if let Some(url) = self.ipfs_url.as_ref() {
            trace!(?url, "Using IPFS URL from command line.");
            Some(url.to_string())
        } else if let Some(url) = env_metadata.and_then(|env| env.ipfs_url()) {
            trace!(url, "Using IPFS URL from environment metadata.");
            Some(url.to_string())
        } else {
            trace!("No default IPFS URL.");
            None
        }
    }

    pub fn username(&self, env_metadata: Option<&Environment>) -> Option<String> {
        trace!("Retrieving username for IpfsOptions.");

        if let Some(username) = self.ipfs_username.as_ref() {
            trace!(?username, "Using IPFS username from command line.");
            Some(username.clone())
        } else if let Some(username) = env_metadata.and_then(|env| env.ipfs_username()) {
            trace!(username, "Using IPFS username from environment metadata.");
            Some(username.to_string())
        } else {
            trace!("No default IPFS username.");
            None
        }
    }

    pub fn password(&self, env_metadata: Option<&Environment>) -> Option<String> {
        trace!("Retrieving password for IpfsOptions.");

        if let Some(password) = self.ipfs_password.as_ref() {
            trace!(?password, "Using IPFS password from command line.");
            Some(password.clone())
        } else if let Some(password) = env_metadata.and_then(|env| env.ipfs_password()) {
            trace!(password, "Using IPFS password from environment metadata.");
            Some(password.to_string())
        } else {
            trace!("No default IPFS password.");
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
        assert_eq!(cmd.options.url(None).unwrap().as_str(), ENV_IPFS_URL);
        assert_eq!(cmd.options.username(None).unwrap(), ENV_IPFS_USERNAME);
        assert_eq!(cmd.options.password(None).unwrap(), ENV_IPFS_PASSWORD);
    }

    #[test]
    fn options_exist_in_env_but_not_in_args() {
        let env_metadata = dojo_world::config::Environment {
            ipfs_url: Some(ENV_IPFS_URL.into()),
            ipfs_username: Some(ENV_IPFS_USERNAME.into()),
            ipfs_password: Some(ENV_IPFS_PASSWORD.into()),
            ..Default::default()
        };

        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), ENV_IPFS_URL);
        assert_eq!(cmd.options.username(Some(&env_metadata)).unwrap().as_str(), ENV_IPFS_USERNAME);
        assert_eq!(cmd.options.password(Some(&env_metadata)).unwrap().as_str(), ENV_IPFS_PASSWORD);
    }

    #[test]
    fn options_doesnt_exist_in_env_but_exist_in_args() {
        let env_metadata = dojo_world::config::Environment::default();
        let cmd = Command::parse_from([
            "sozo",
            "--ipfs-url",
            ENV_IPFS_URL,
            "--ipfs-username",
            ENV_IPFS_USERNAME,
            "--ipfs-password",
            ENV_IPFS_PASSWORD,
        ]);

        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), ENV_IPFS_URL);
        assert_eq!(cmd.options.username(Some(&env_metadata)).unwrap().as_str(), ENV_IPFS_USERNAME);
        assert_eq!(cmd.options.password(Some(&env_metadata)).unwrap().as_str(), ENV_IPFS_PASSWORD);
    }

    #[test]
    fn options_exists_in_both() {
        let env_metadata = dojo_world::config::Environment {
            ipfs_url: Some(ENV_IPFS_URL.into()),
            ipfs_username: Some(ENV_IPFS_USERNAME.into()),
            ipfs_password: Some(ENV_IPFS_PASSWORD.into()),
            ..Default::default()
        };

        let cmd = Command::parse_from([
            "sozo",
            "--ipfs-url",
            ENV_IPFS_URL,
            "--ipfs-username",
            ENV_IPFS_USERNAME,
            "--ipfs-password",
            ENV_IPFS_PASSWORD,
        ]);

        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), ENV_IPFS_URL);
        assert_eq!(cmd.options.username(Some(&env_metadata)).unwrap().as_str(), ENV_IPFS_USERNAME);
        assert_eq!(cmd.options.password(Some(&env_metadata)).unwrap().as_str(), ENV_IPFS_PASSWORD);
    }

    #[test]
    fn url_exists_in_neither() {
        let env_metadata = dojo_world::config::Environment::default();
        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.options.url(Some(&env_metadata)), None);
        assert_eq!(cmd.options.username(Some(&env_metadata)), None);
        assert_eq!(cmd.options.password(Some(&env_metadata)), None);
    }
}
