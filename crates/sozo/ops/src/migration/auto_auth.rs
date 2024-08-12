use anyhow::Result;
use dojo_world::contracts::WorldContract;
use dojo_world::migration::TxnConfig;
use scarb::core::Workspace;
use starknet::accounts::ConnectedAccount;

use crate::auth::{grant_writer, revoke_writer, ResourceWriter};

pub async fn auto_authorize<A>(
    ws: &Workspace<'_>,
    world: &WorldContract<A>,
    txn_config: &TxnConfig,
    default_namespace: &str,
    grant: &[ResourceWriter],
    revoke: &[ResourceWriter],
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
    A::SignError: 'static,
{
    let ui = ws.config().ui();

    grant_writer(&ui, world, grant, *txn_config, default_namespace).await?;
    revoke_writer(&ui, world, revoke, *txn_config, default_namespace).await?;

    Ok(())
}
