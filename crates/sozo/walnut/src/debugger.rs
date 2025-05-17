use dojo_utils::TransactionResult;
use scarb_metadata::Metadata;
use scarb_ui::Ui;
use url::Url;

use crate::Error;
use crate::transaction::walnut_debug_transaction;
use crate::verification::walnut_verify;

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
    pub fn debug_transaction(
        &self,
        ui: &Ui,
        transaction_result: &TransactionResult,
    ) -> Result<(), Error> {
        let transaction_hash = match transaction_result {
            TransactionResult::Hash(transaction_hash) => transaction_hash,
            TransactionResult::Noop => {
                return Ok(());
            }
            TransactionResult::HashReceipt(transaction_hash, _) => transaction_hash,
        };
        let url = walnut_debug_transaction(&self.rpc_url, transaction_hash)?;
        ui.print(format!("Debug transaction with Walnut: {url}"));
        Ok(())
    }

    /// Verifies a migration strategy with Walnut by uploading the source code of the contracts and
    /// models in the strategy.
    pub async fn verify(scarb_metadata: &Metadata, ui: &Ui) -> anyhow::Result<()> {
        walnut_verify(scarb_metadata, ui).await
    }
}
