use katana_primitives::transaction::{ExecutableTxWithHash, L1HandlerTx};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MsgFromL1(starknet::core::types::MsgFromL1);

impl From<MsgFromL1> for ExecutableTxWithHash {
    fn from(value: MsgFromL1) -> Self {
        let tx = L1HandlerTx {
            calldata: value.0.payload,
            contract_address: value.0.to_address.into(),
            entry_point_selector: value.0.entry_point_selector,
            ..Default::default()
        };
        let hash = tx.calculate_hash();
        ExecutableTxWithHash { hash, transaction: tx.into() }
    }
}
