use std::str::FromStr;

use anyhow::Result;
use clap::{Args, Subcommand};
use dojo_world::contracts::cairo_utils;
use dojo_world::metadata::dojo_metadata_from_workspace;
use scarb::core::Config;
use starknet_crypto::FieldElement;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::ops::auth;

#[derive(Debug, Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelContract {
    pub model: FieldElement,
    pub contract: String,
}

impl FromStr for ModelContract {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();

        let (model, contract) = match parts.as_slice() {
            [model, contract] => (model, contract),
            _ => anyhow::bail!(
                "Model and contract address are expected to be comma separated: `sozo auth writer \
                 model_name,0x1234`"
            ),
        };

        let model = cairo_utils::str_to_felt(model)
            .map_err(|_| anyhow::anyhow!("Invalid model name: {}", model))?;

        Ok(ModelContract { model, contract: contract.to_string() })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceType {
    Contract(String),
    Model(FieldElement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OwnerResource {
    pub resource: ResourceType,
    pub owner: FieldElement,
}

impl FromStr for OwnerResource {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();

        let (resource_part, owner_part) = match parts.as_slice() {
            [resource, owner] => (*resource, *owner),
            _ => anyhow::bail!(
                "Owner and resource are expected to be comma separated: `sozo auth owner \
                 resource_type:resource_name,0x1234`"
            ),
        };

        let owner = FieldElement::from_hex_be(owner_part)
            .map_err(|_| anyhow::anyhow!("Invalid owner address: {}", owner_part))?;

        let resource_parts = resource_part.split_once(':');
        let resource = match resource_parts {
            Some(("contract", name)) => ResourceType::Contract(name.to_string()),
            Some(("model", name)) => {
                let model = cairo_utils::str_to_felt(name)
                    .map_err(|_| anyhow::anyhow!("Invalid model name: {}", name))?;
                ResourceType::Model(model)
            }
            _ => anyhow::bail!(
                "Resource is expected to be in the format `resource_type:resource_name`: `sozo \
                 auth owner 0x1234,resource_type:resource_name`"
            ),
        };

        Ok(OwnerResource { owner, resource })
    }
}

#[derive(Debug, Subcommand)]
pub enum AuthKind {
    #[command(about = "Grant a contract permission to write to a model.")]
    Writer {
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "model,contract_address")]
        #[arg(help = "A list of models and contract address to grant write access to. Comma \
                      separated values to indicate model name and contract address e.g. \
                      model_name,path::to::contract model_name,contract_address ")]
        models_contracts: Vec<ModelContract>,
    },
    #[command(about = "Grant ownership of a resource.")]
    Owner {
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "resource,owner_address")]
        #[arg(help = "A list of owners and resources to grant ownership to. Comma separated \
                      values to indicate owner address and resouce e.g. \
                      contract:path::to::contract,0x1234 contract:contract_address,0x1111, \
                      model:model_name,0xbeef")]
        owners_resources: Vec<OwnerResource>,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    #[command(about = "Grant an auth role.")]
    Grant {
        #[command(subcommand)]
        kind: AuthKind,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[command(flatten)]
        account: AccountOptions,

        #[command(flatten)]
        transaction: TransactionOptions,
    },
    #[command(about = "Revoke an auth role.")]
    Revoke {
        #[command(subcommand)]
        kind: AuthKind,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[command(flatten)]
        account: AccountOptions,

        #[command(flatten)]
        transaction: TransactionOptions,
    },
}

impl AuthArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let env_metadata = if config.manifest_path().exists() {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

            dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
        } else {
            None
        };

        config.tokio_handle().block_on(auth::execute(self.command, env_metadata))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use starknet_crypto::FieldElement;

    use super::*;

    #[test]
    fn test_owner_resource_from_str() {
        // Test valid input
        let input = "contract:path::to::contract,0x1234";
        let expected_owner = FieldElement::from_hex_be("0x1234").unwrap();
        let expected_resource = ResourceType::Contract("path::to::contract".to_string());
        let expected = OwnerResource { owner: expected_owner, resource: expected_resource };
        let result = OwnerResource::from_str(input).unwrap();
        assert_eq!(result, expected);

        // Test valid input with model
        let input = "model:model_name,0x1234";
        let expected_owner = FieldElement::from_hex_be("0x1234").unwrap();
        let expected_model = cairo_utils::str_to_felt("model_name").unwrap();
        let expected_resource = ResourceType::Model(expected_model);
        let expected = OwnerResource { owner: expected_owner, resource: expected_resource };
        let result = OwnerResource::from_str(input).unwrap();
        assert_eq!(result, expected);

        // Test invalid input
        let input = "invalid_input";
        let result = OwnerResource::from_str(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_model_contract_from_str() {
        // Test valid input
        let input = "model_name,0x1234";
        let expected_model = cairo_utils::str_to_felt("model_name").unwrap();
        let expected_contract = "0x1234";
        let expected =
            ModelContract { model: expected_model, contract: expected_contract.to_string() };
        let result = ModelContract::from_str(input).unwrap();
        assert_eq!(result, expected);

        // Test invalid input
        let input = "invalid_input";
        let result = ModelContract::from_str(input);
        assert!(result.is_err());
    }
}
