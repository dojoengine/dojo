use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::Args;
use dojo_world::metadata::Environment;
use starknet::core::types::FieldElement;
use tracing::trace;

use super::DOJO_WORLD_ADDRESS_ENV_VAR;

#[derive(Debug, Args)]
#[command(next_help_heading = "World options")]
pub struct WorldOptions {
    #[arg(help = "The address of the World contract.")]
    #[arg(long = "world", env = DOJO_WORLD_ADDRESS_ENV_VAR)]
    #[arg(global = true)]
    pub world_address: Option<FieldElement>,
}

impl WorldOptions {
    pub fn address(&self, env_metadata: Option<&Environment>) -> Result<FieldElement> {
        if let Some(world_address) = self.world_address {
            trace!(?world_address, "Loaded world_address.");
            Ok(world_address)
        } else if let Some(world_address) = env_metadata.and_then(|env| env.world_address()) {
            trace!(world_address, "Loaded world_address from env metadata.");
            Ok(FieldElement::from_str(world_address)?)
        } else {
            Err(anyhow!(
                "Could not find World address. Please specify it with --world, environment \
                 variable or in the world config."
            ))
        }
    }
}

#[cfg(test)]
mod tests {

    use clap::Parser;
    use starknet_crypto::FieldElement;

    use super::{WorldOptions, DOJO_WORLD_ADDRESS_ENV_VAR};

    #[derive(clap::Parser, Debug)]
    struct Command {
        #[clap(flatten)]
        pub inner: WorldOptions,
    }
    #[test]

    fn world_address_read_from_env_variable() {
        std::env::set_var(DOJO_WORLD_ADDRESS_ENV_VAR, "0x0");

        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.inner.world_address, Some(FieldElement::from_hex_be("0x0").unwrap()));
    }

    #[test]
    fn world_address_from_args() {
        let cmd = Command::parse_from(["sozo", "--world", "0x0"]);
        assert_eq!(cmd.inner.address(None).unwrap(), FieldElement::from_hex_be("0x0").unwrap());
    }

    #[test]
    fn world_address_from_env_metadata() {
        let env_metadata = dojo_world::metadata::Environment {
            world_address: Some("0x0".to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from([""]);
        assert_eq!(
            cmd.inner.address(Some(&env_metadata)).unwrap(),
            FieldElement::from_hex_be("0x0").unwrap()
        );
    }

    #[test]
    fn world_address_from_both() {
        let env_metadata = dojo_world::metadata::Environment {
            world_address: Some("0x0".to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--world", "0x1"]);
        assert_eq!(
            cmd.inner.address(Some(&env_metadata)).unwrap(),
            FieldElement::from_hex_be("0x1").unwrap()
        );
    }

    #[test]
    fn world_address_from_neither() {
        let cmd = Command::parse_from([""]);
        assert!(cmd.inner.address(None).is_err());
    }
}
