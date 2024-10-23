use katana_primitives::chain::ChainId;
use katana_primitives::transaction::L1HandlerTx;
use katana_primitives::utils::transaction::compute_l2_to_l1_message_hash;
use katana_primitives::Felt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgFromL1(starknet::core::types::MsgFromL1);

impl MsgFromL1 {
    pub fn into_tx_with_chain_id(self, chain_id: ChainId) -> L1HandlerTx {
        // Set the L1 to L2 message nonce to 0, because this is just used
        // for the `estimateMessageFee` RPC.
        let nonce = Felt::ZERO;

        // When executing a l1 handler tx, blockifier just assert that the paid_fee_on_l1 is
        // anything but 0. See: https://github.com/dojoengine/sequencer/blob/d6951f24fc2082c7aa89cdbc063648915b131d74/crates/blockifier/src/transaction/transaction_execution.rs#L140-L145
        //
        // For fee estimation, this value is basically irrelevant.
        let paid_fee_on_l1 = 1u128;

        let message_hash = compute_l2_to_l1_message_hash(
            // This conversion will never fail bcs `from_address` is 20 bytes and the it will only
            // fail if the slice is > 32 bytes
            Felt::from_bytes_be_slice(self.0.from_address.as_bytes()),
            self.0.to_address,
            &self.0.payload,
        );

        // In an l1_handler transaction, the first element of the calldata is always the Ethereum
        // address of the sender (msg.sender). https://docs.starknet.io/documentation/architecture_and_concepts/Network_Architecture/messaging-mechanism/#l1-l2-messages
        let mut calldata = vec![Felt::from(self.0.from_address)];
        calldata.extend(self.0.payload);

        L1HandlerTx {
            nonce,
            chain_id,
            calldata,
            message_hash,
            paid_fee_on_l1,
            version: Felt::ZERO,
            contract_address: self.0.to_address.into(),
            entry_point_selector: self.0.entry_point_selector,
        }
    }
}
