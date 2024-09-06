use anyhow::Result;
use dojo_utils::TxnConfig;
use dojo_world::contracts::WorldContract;
use scarb::core::Workspace;
use starknet::accounts::ConnectedAccount;
use url::Url;

use crate::auth::{grant_writer, revoke_writer, ResourceWriter};

pub async fn auto_authorize<A>(
    ws: &Workspace<'_>,
    world: &WorldContract<A>,
    txn_config: &TxnConfig,
    default_namespace: &str,
    grant: &[ResourceWriter],
    revoke: &[ResourceWriter],
    rpc_url: &Option<Url>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
    A::SignError: 'static,
{
    let ui = ws.config().ui();

    // Disable the walnut flag
    let txn_config_without_walnut = TxnConfig { walnut: false, ..*txn_config };

    grant_writer(&ui, world, grant, txn_config_without_walnut, default_namespace, rpc_url).await?;
    revoke_writer(&ui, world, revoke, txn_config_without_walnut, default_namespace, rpc_url).await?;

    Ok(())
}
