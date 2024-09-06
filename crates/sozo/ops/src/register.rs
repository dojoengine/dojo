use std::collections::HashMap;

use anyhow::{Context, Result};
use dojo_utils::{TransactionExt, TxnConfig};
use dojo_world::contracts::{WorldContract, WorldContractReader};
use dojo_world::manifest::DeploymentManifest;
use scarb::core::Config;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::Felt;
use starknet::providers::Provider;

use crate::utils::handle_transaction_result;

pub async fn model_register<A, P>(
    models: Vec<Felt>,
    world: &WorldContract<A>,
    txn_config: TxnConfig,
    world_reader: WorldContractReader<P>,
    world_address: Felt,
    config: &Config,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
    P: Provider + Sync + Send,
{
    Ok(())
}
