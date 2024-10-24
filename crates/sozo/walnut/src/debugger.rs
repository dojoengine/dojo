use scarb::core::Workspace;
use scarb_ui::Ui;
use starknet::core::types::Felt;
use url::Url;

use crate::transaction::walnut_debug_transaction;
use crate::verification::walnut_verify_migration_strategy;
use crate::{utils, Error};

/// A debugger for Starknet transactions embedding the walnut configuration.
#[derive(Debug)]
pub struct WalnutDebugger {
    rpc_url: Url,
}

impl WalnutDebugger {
    /// Creates a new Walnut debugger.
    pub fn new(rpc_url: Url) -> Self {
        Self { rpc_url }
    }

    /// Creates a new Walnut debugger if the `use_walnut` flag is set.
    pub fn new_from_flag(use_walnut: bool, rpc_url: Url) -> Option<Self> {
        if use_walnut { Some(Self::new(rpc_url)) } else { None }
    }

    /// Debugs a transaction with Walnut by printing a link to the Walnut debugger page.
    pub fn debug_transaction(&self, ui: &Ui, transaction_hash: &Felt) -> Result<(), Error> {
        let url = walnut_debug_transaction(&self.rpc_url, transaction_hash)?;
        ui.print(format!("Debug transaction with Walnut: {url}"));
        Ok(())
    }

    /// Verifies a migration strategy with Walnut by uploading the source code of the contracts and
    /// models in the strategy.
    pub async fn verify_migration_strategy(
        &self,
        ws: &Workspace<'_>,
        strategy: &MigrationStrategy,
    ) -> anyhow::Result<()> {
        walnut_verify_migration_strategy(ws, self.rpc_url.to_string(), strategy).await
    }

    /// Checks if the Walnut API key is set.
    pub fn check_api_key() -> Result<(), Error> {
        let _ = utils::walnut_get_api_key()?;
        Ok(())
    }
}
