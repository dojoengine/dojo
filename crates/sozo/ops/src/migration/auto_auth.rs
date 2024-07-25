use std::{collections::HashMap, str::FromStr};

use anyhow::Result;
use dojo_world::manifest::ContractMetadata;
use dojo_world::migration::TxnConfig;
use dojo_world::{contracts::WorldContract, manifest::Operation};
use scarb::core::Workspace;
use scarb_ui::Ui;
use starknet::accounts::ConnectedAccount;

use super::ui::MigrationUi;
use crate::auth::{grant_writer, revoke_writer, ResourceWriter};

pub async fn auto_authorize<A>(
    ws: &Workspace<'_>,
    world: &WorldContract<A>,
    txn_config: &TxnConfig,
    default_namespace: &str,
    work: &Vec<(String, ContractMetadata)>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
    A::SignError: 'static,
{
    let ui = ws.config().ui();

    ui.print(" ");
    ui.print_step(6, "🖋️", "Authorizing systems based on overlay...");
    ui.print(" ");
    let (grant, revoke) = create_writers(&ui, work)?;
    grant_writer(&ui, world, grant, *txn_config, default_namespace).await?;
    revoke_writer(&ui, world, revoke, *txn_config, default_namespace).await?;

    Ok(())
}

pub fn create_writers(
    ui: &Ui,
    work: &Vec<(String, ContractMetadata)>,
) -> Result<(Vec<ResourceWriter>, Vec<ResourceWriter>)> {
    let mut grant = HashMap::new();
    let mut revoke = HashMap::new();

    // From all the contracts that were migrated successfully.
    for (tag, contract_metadata) in work {
        // separate out the resources that are being granted and revoked.
        // based on the Operation type in the contract_metadata.
        for (resource, operation) in contract_metadata {
            match operation {
                Operation::Grant => {
                    let entry = grant.entry(tag).or_insert(vec![]);
                    entry.push(resource);
                }
                Operation::Revoke => {
                    let entry = revoke.entry(tag).or_insert(vec![]);
                    entry.push(resource);
                }
            }
        }
    }

    let mut grant_writer = vec![];
    for (tag, resources) in grant.iter() {
        ui.print_sub(format!("Authorizing write access of {} for resources: {:?}", tag, resources));

        for resource in resources {
            let resource = if resource.contains(':') {
                resource.to_string()
            } else {
                // Default to model if no prefix was given.
                format!("m:{}", resource)
            };
            let resource = format!("{},{}", resource, tag);
            grant_writer.push(ResourceWriter::from_str(&resource)?);
        }
    }

    let mut revoke_writer = vec![];
    for (tag, resources) in revoke.iter() {
        ui.print_sub(format!("Revoking write access of {} for resources: {:?}", tag, resources));

        for resource in resources {
            let resource = if resource.contains(':') {
                resource.to_string()
            } else {
                // Default to model if no prefix was given.
                format!("m:{}", resource)
            };
            let resource = format!("{},{}", resource, tag);
            revoke_writer.push(ResourceWriter::from_str(&resource)?);
        }
    }

    Ok((grant_writer, revoke_writer))
}
