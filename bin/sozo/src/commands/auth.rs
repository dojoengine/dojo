use anyhow::Result;
use clap::{Args, Subcommand};
use dojo_world::metadata::Environment;
use scarb::core::Config;
use scarb_ui::Ui;
use sozo_ops::auth;
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
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
        trace!(args = ?self);

        let env_metadata = utils::load_metadata_from_config(config)?;
        trace!(metadata=?env_metadata, "Loaded environment.");

        match self.command {
            AuthCommand::Grant { kind, world, starknet, account, transaction } => {
                config.tokio_handle().block_on(grant(
                    &config.ui(),
                    world,
                    account,
                    starknet,
                    env_metadata,
                    kind,
                    transaction,
                    config,
                ))
            }
            AuthCommand::Revoke { kind, world, starknet, account, transaction } => {
                config.tokio_handle().block_on(revoke(
                    &config.ui(),
                    world,
                    account,
                    starknet,
                    env_metadata,
                    kind,
                    transaction,
                    config,
                ))
            }
        }
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
        models_contracts: Vec<auth::ModelContract>,
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
        owners_resources: Vec<auth::OwnerResource>,
    },
}

#[allow(clippy::too_many_arguments)]
pub async fn grant(
    ui: &Ui,
    world: WorldOptions,
    account: AccountOptions,
    starknet: StarknetOptions,
    env_metadata: Option<Environment>,
    kind: AuthKind,
    transaction: TransactionOptions,
    config: &Config,
) -> Result<()> {
    trace!(?kind, ?world, ?starknet, ?account, ?transaction, "Executing Grant command.");
    let world =
        utils::world_from_env_metadata(world, account, starknet, &env_metadata, config).await?;

    match kind {
        AuthKind::Writer { models_contracts } => {
            trace!(
                contracts=?models_contracts,
                "Granting Writer permissions."
            );
            auth::grant_writer(ui, &world, models_contracts, transaction.into()).await
        }
        AuthKind::Owner { owners_resources } => {
            trace!(
                resources=?owners_resources,
                "Granting Owner permissions."
            );
            auth::grant_owner(ui, &world, owners_resources, transaction.into()).await
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn revoke(
    ui: &Ui,
    world: WorldOptions,
    account: AccountOptions,
    starknet: StarknetOptions,
    env_metadata: Option<Environment>,
    kind: AuthKind,
    transaction: TransactionOptions,
    config: &Config,
) -> Result<()> {
    trace!(?kind, ?world, ?starknet, ?account, ?transaction, "Executing Revoke command.");
    let world =
        utils::world_from_env_metadata(world, account, starknet, &env_metadata, config).await?;

    match kind {
        AuthKind::Writer { models_contracts } => {
            trace!(
                contracts=?models_contracts,
                "Revoking Writer permissions."
            );
            auth::revoke_writer(ui, &world, models_contracts, transaction.into()).await
        }
        AuthKind::Owner { owners_resources } => {
            trace!(
                resources=?owners_resources,
                "Revoking Owner permissions."
            );
            auth::revoke_owner(ui, &world, owners_resources, transaction.into()).await
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use dojo_world::contracts::cairo_utils;
    use starknet_crypto::FieldElement;

    use super::*;

    #[test]
    fn test_owner_resource_from_str() {
        // Test valid input
        let input = "contract:path::to::contract,0x1234";
        let expected_owner = FieldElement::from_hex_be("0x1234").unwrap();
        let expected_resource = auth::ResourceType::Contract("path::to::contract".to_string());
        let expected = auth::OwnerResource { owner: expected_owner, resource: expected_resource };
        let result = auth::OwnerResource::from_str(input).unwrap();
        assert_eq!(result, expected);

        // Test valid input with model
        let input = "model:model_name,0x1234";
        let expected_owner = FieldElement::from_hex_be("0x1234").unwrap();
        let expected_model = cairo_utils::str_to_felt("model_name").unwrap();
        let expected_resource = auth::ResourceType::Model(expected_model);
        let expected = auth::OwnerResource { owner: expected_owner, resource: expected_resource };
        let result = auth::OwnerResource::from_str(input).unwrap();
        assert_eq!(result, expected);

        // Test invalid input
        let input = "invalid_input";
        let result = auth::OwnerResource::from_str(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_model_contract_from_str() {
        // Test valid input
        let input = "model_name,0x1234";
        let expected_model = cairo_utils::str_to_felt("model_name").unwrap();
        let expected_contract = "0x1234";
        let expected =
            auth::ModelContract { model: expected_model, contract: expected_contract.to_string() };
        let result = auth::ModelContract::from_str(input).unwrap();
        assert_eq!(result, expected);

        // Test invalid input
        let input = "invalid_input";
        let result = auth::ModelContract::from_str(input);
        assert!(result.is_err());
    }
}
