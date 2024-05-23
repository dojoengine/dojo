use katana_primitives::chain::ChainId;
use katana_primitives::transaction::L1HandlerTx;
use katana_primitives::utils::transaction::compute_l2_to_l1_message_hash;
use katana_primitives::FieldElement;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgFromL1(starknet::core::types::MsgFromL1);

impl MsgFromL1 {
    pub fn into_tx_with_chain_id(self, chain_id: ChainId) -> L1HandlerTx {
        // Set the L1 to L2 message nonce to 0, because this is just used
        // for the `estimateMessageFee` RPC.
        let nonce = FieldElement::ZERO;

        let message_hash = compute_l2_to_l1_message_hash(
            // This conversion will never fail bcs `from_address` is 20 bytes and the it will only
            // fail if the slice is > 32 bytes
            FieldElement::from_byte_slice_be(self.0.from_address.as_bytes()).unwrap(),
            self.0.to_address,
            &self.0.payload,
        );

        L1HandlerTx {
            nonce,
            chain_id,
            message_hash,
            calldata: self.0.payload,
            version: FieldElement::ZERO,
            paid_fee_on_l1: Default::default(),
            contract_address: self.0.to_address.into(),
            entry_point_selector: self.0.entry_point_selector,
        }
    }
}
