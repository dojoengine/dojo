use anyhow::Result;
use clap::{Args, Subcommand};
use dojo_world::config::Environment;
use dojo_world::metadata::get_default_namespace_from_ws;
use scarb::core::Config;
use scarb_ui::Ui;
use sozo_ops::auth;
use sozo_walnut::WalnutDebugger;
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

        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        let default_namespace = get_default_namespace_from_ws(&ws)?;

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
                    &default_namespace,
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
                    &default_namespace,
                ))
            }
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum AuthKind {
    #[command(about = "Grant a contract permission to write to a resource (contract, model or \
                       namespace).")]
    Writer {
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "resource,contract_tag_or_address")]
        #[arg(help = "A list of resource/contract couples to grant write access to.
Comma separated values to indicate resource identifier and contract tag or address.
A resource identifier must use the following format: \
                      <contract|c|namespace|ns|model|m>:<tag_or_name>.\n
Some examples:
   model:dojo_examples-Moves,0x1234
   m:Moves,0x1234
   ns:dojo_examples,actions
")]
        models_contracts: Vec<auth::ResourceWriter>,
    },
    #[command(about = "Grant ownership of a resource (contract, model or namespace).")]
    Owner {
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "resource,owner_address")]
        #[arg(help = "A list of resources and owners to grant ownership to.
Comma separated values to indicate resource identifier and owner address.
A resource identifier must use the following format: \
                      <contract|c|namespace|ns|model|m>:<tag_or_name>.\n
Some examples:
   model:dojo_examples-Moves,0x1234
   m:Moves,0x1234
   ns:dojo_examples,0xbeef
")]
        owners_resources: Vec<auth::ResourceOwner>,
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
    default_namespace: &str,
) -> Result<()> {
    trace!(?kind, ?world, ?starknet, ?account, ?transaction, "Executing Grant command.");
    let world =
        utils::world_from_env_metadata(world, account, &starknet, &env_metadata, config).await?;

    let walnut_debugger =
        WalnutDebugger::new_from_flag(transaction.walnut, starknet.url(env_metadata.as_ref())?);

    match kind {
        AuthKind::Writer { models_contracts } => {
            trace!(
                contracts=?models_contracts,
                "Granting Writer permissions."
            );
            auth::grant_writer(
                ui,
                &world,
                &models_contracts,
                &transaction.into(),
                default_namespace,
                &walnut_debugger,
            )
            .await
        }
        AuthKind::Owner { owners_resources } => {
            trace!(
                resources=?owners_resources,
                "Granting Owner permissions."
            );
            auth::grant_owner(
                ui,
                &world,
                &owners_resources,
                &transaction.into(),
                default_namespace,
                &walnut_debugger,
            )
            .await
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
    default_namespace: &str,
) -> Result<()> {
    trace!(?kind, ?world, ?starknet, ?account, ?transaction, "Executing Revoke command.");
    let world =
        utils::world_from_env_metadata(world, account, &starknet, &env_metadata, config).await?;

    let walnut_debugger =
        WalnutDebugger::new_from_flag(transaction.walnut, starknet.url(env_metadata.as_ref())?);

    match kind {
        AuthKind::Writer { models_contracts } => {
            trace!(
                contracts=?models_contracts,
                "Revoking Writer permissions."
            );
            auth::revoke_writer(
                ui,
                &world,
                &models_contracts,
                &transaction.into(),
                default_namespace,
                &walnut_debugger,
            )
            .await
        }
        AuthKind::Owner { owners_resources } => {
            trace!(
                resources=?owners_resources,
                "Revoking Owner permissions."
            );
            auth::revoke_owner(
                ui,
                &world,
                &owners_resources,
                &transaction.into(),
                default_namespace,
                &walnut_debugger,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use starknet::core::types::Felt;

    use super::*;

    #[test]
    fn test_resource_type_from_str() {
        let inputs = [
            (
                "contract:name:contract_name",
                auth::ResourceType::Contract("name:contract_name".to_string()),
            ),
            ("c:0x1234", auth::ResourceType::Contract("0x1234".to_string())),
            ("model:name:model_name", auth::ResourceType::Model("name:model_name".to_string())),
            ("m:name:model_name", auth::ResourceType::Model("name:model_name".to_string())),
            (
                "namespace:namespace_name",
                auth::ResourceType::Namespace("namespace_name".to_string()),
            ),
            ("ns:namespace_name", auth::ResourceType::Namespace("namespace_name".to_string())),
        ];

        for (input, expected) in inputs {
            let res = auth::ResourceType::from_str(input);
            assert!(res.is_ok(), "Unable to parse input '{input}'");

            let resource = res.unwrap();
            assert!(
                resource == expected,
                "Wrong resource type: expected: {:#?} got: {:#?}",
                expected,
                resource
            );
        }
    }

    #[test]
    fn test_resource_type_from_str_bad_resource_identifier() {
        let input = "other:model_name";
        let res = auth::ResourceType::from_str(input);
        assert!(res.is_err(), "Bad identifier: This resource should not be accepted: '{input}'");
    }

    #[test]
    fn test_resource_type_from_str_bad_resource_format() {
        let input = "model_name";
        let res = auth::ResourceType::from_str(input);
        assert!(res.is_err(), "Bad format: This resource should not be accepted: '{input}'");
    }

    #[test]
    fn test_resource_writer_from_str() {
        let inputs = [
            (
                "model:name:model_name,name:contract_name",
                auth::ResourceWriter {
                    resource: auth::ResourceType::Model("name:model_name".to_string()),
                    tag_or_address: "name:contract_name".to_string(),
                },
            ),
            (
                "ns:namespace_name,0x1234",
                auth::ResourceWriter {
                    resource: auth::ResourceType::Namespace("namespace_name".to_string()),
                    tag_or_address: "0x1234".to_string(),
                },
            ),
        ];

        for (input, expected) in inputs {
            let res = auth::ResourceWriter::from_str(input);
            assert!(res.is_ok(), "Unable to parse input '{input}'");

            let writer = res.unwrap();
            assert!(
                writer == expected,
                "Wrong resource writer: expected: {:#?} got: {:#?}",
                expected,
                writer
            );
        }
    }

    #[test]
    fn test_resource_writer_from_str_bad_format() {
        let input = "model_name";
        let res = auth::ResourceWriter::from_str(input);
        assert!(res.is_err(), "Bad format: This resource writer should not be accepted: '{input}'");
    }

    #[test]
    fn test_resource_writer_from_str_bad_owner_address() {
        let input = "model:model_name:bad_address";
        let res = auth::ResourceWriter::from_str(input);
        assert!(
            res.is_err(),
            "Bad address: This resource writer should not be accepted: '{input}'"
        );
    }

    #[test]
    fn test_resource_owner_from_str() {
        let inputs = [
            (
                "model:name:model_name,0x1234",
                auth::ResourceOwner {
                    resource: auth::ResourceType::Model("name:model_name".to_string()),
                    owner: Felt::from_hex("0x1234").unwrap(),
                },
            ),
            (
                "ns:namespace_name,0x1111",
                auth::ResourceOwner {
                    resource: auth::ResourceType::Namespace("namespace_name".to_string()),
                    owner: Felt::from_hex("0x1111").unwrap(),
                },
            ),
        ];

        for (input, expected) in inputs {
            let res = auth::ResourceOwner::from_str(input);
            assert!(res.is_ok(), "Unable to parse input '{input}'");

            let owner = res.unwrap();
            assert!(
                owner == expected,
                "Wrong resource owner: expected: {:#?} got: {:#?}",
                expected,
                owner
            );
        }
    }

    #[test]
    fn test_resource_owner_from_str_bad_format() {
        let input = "model_name";
        let res = auth::ResourceOwner::from_str(input);
        assert!(res.is_err(), "Bad format: This resource owner should not be accepted: '{input}'");
    }

    #[test]
    fn test_resource_owner_from_str_bad_owner_address() {
        let input = "model:model_name:bad_address";
        let res = auth::ResourceOwner::from_str(input);
        assert!(res.is_err(), "Bad address: This resource owner should not be accepted: '{input}'");
    }
}
