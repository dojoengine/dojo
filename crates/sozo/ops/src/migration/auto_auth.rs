use anyhow::Result;
use dojo_utils::TxnConfig;
use dojo_world::contracts::WorldContract;
use scarb::core::Workspace;
#[cfg(feature = "walnut")]
use sozo_walnut::WalnutDebugger;
use starknet::accounts::ConnectedAccount;

use crate::auth::{grant_writer, revoke_writer, ResourceWriter};

pub async fn auto_authorize<A>(
    ws: &Workspace<'_>,
    world: &WorldContract<A>,
    txn_config: &TxnConfig,
    default_namespace: &str,
    grant: &[ResourceWriter],
    revoke: &[ResourceWriter],
    #[cfg(feature = "walnut")] walnut_debugger: &Option<WalnutDebugger>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
    A::SignError: 'static,
{
    let ui = ws.config().ui();

    grant_writer(
        &ui,
        world,
        grant,
        txn_config,
        default_namespace,
        #[cfg(feature = "walnut")]
        walnut_debugger,
    )
    .await?;
    revoke_writer(
        &ui,
        world,
        revoke,
        txn_config,
        default_namespace,
        #[cfg(feature = "walnut")]
        walnut_debugger,
    )
    .await?;

    Ok(())
}
