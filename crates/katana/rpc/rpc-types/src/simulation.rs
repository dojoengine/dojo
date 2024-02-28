use anyhow::Result;
use katana_primitives::{
    block::BlockIdOrTag,
    chain::ChainId,
    transaction::{ExecutableTx, ExecutableTxWithHash},
};
use serde::{Deserialize, Serialize};

use crate::{transaction::BroadcastedTx, SimulationFlag};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateTransactionsRequest {
    pub block_id: BlockIdOrTag,
    pub transactions: Vec<BroadcastedTx>,
    pub simulation_flags: Vec<SimulationFlag>,
}

impl SimulateTransactionsRequest {
    /// Converts the request into a vector of executable transactions.
    pub fn try_into_executables_with_hash(
        &self,
        chain_id: ChainId,
    ) -> Result<Vec<ExecutableTxWithHash>> {
        self.transactions
            .clone()
            .into_iter()
            .map(|t| {
                Ok(match t {
                    BroadcastedTx::Invoke(tx) => ExecutableTxWithHash::new(ExecutableTx::Invoke(
                        tx.into_tx_with_chain_id(chain_id),
                    )),
                    BroadcastedTx::Declare(tx) => ExecutableTxWithHash::new(ExecutableTx::Declare(
                        tx.try_into_tx_with_chain_id(chain_id)?,
                    )),
                    BroadcastedTx::DeployAccount(tx) => ExecutableTxWithHash::new(
                        ExecutableTx::DeployAccount(tx.into_tx_with_chain_id(chain_id)),
                    ),
                })
            })
            .collect()
    }
}
