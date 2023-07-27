use std::sync::Arc;

use starknet::core::{
    crypto::compute_hash_on_elements,
    types::{BlockStatus, FieldElement},
};

use crate::backend::pending::ExecutedTransaction;

use super::transaction::TransactionOutput;

/// Header of a pending block
#[derive(Debug)]
pub struct PartialHeader {
    pub parent_hash: FieldElement,
    pub number: u64,
    pub gas_price: u128,
    pub timestamp: u64,
    pub sequencer_address: FieldElement,
}

#[derive(Debug, Clone)]
pub struct Header {
    pub parent_hash: FieldElement,
    pub number: u64,
    pub gas_price: u128,
    pub timestamp: u64,
    pub state_root: FieldElement,
    pub sequencer_address: FieldElement,
}

impl Header {
    pub fn new(partial_header: PartialHeader, state_root: FieldElement) -> Self {
        Self {
            state_root,
            number: partial_header.number,
            gas_price: partial_header.gas_price,
            timestamp: partial_header.timestamp,
            parent_hash: partial_header.parent_hash,
            sequencer_address: partial_header.sequencer_address,
        }
    }

    pub fn hash(&self) -> FieldElement {
        compute_hash_on_elements(&vec![
            self.number.into(),     // block number
            self.state_root,        // state root
            self.sequencer_address, // sequencer address
            self.timestamp.into(),  // block timestamp
            FieldElement::ZERO,     // transaction commitment
            FieldElement::ZERO,     // event commitment
            FieldElement::ZERO,     // protocol version
            FieldElement::ZERO,     // extra data
            self.parent_hash,       // parent hash
        ])
    }
}

#[derive(Debug)]
pub struct Block {
    pub header: Header,
    pub status: BlockStatus,
    pub transactions: Vec<Arc<ExecutedTransaction>>,
    pub outputs: Vec<TransactionOutput>,
}

impl Block {
    pub fn new(
        partial_header: PartialHeader,
        transactions: Vec<Arc<ExecutedTransaction>>,
        outputs: Vec<TransactionOutput>,
    ) -> Self {
        // TODO: compute state root
        let state_root = FieldElement::ZERO;

        Self {
            header: Header::new(partial_header, state_root),
            status: BlockStatus::AcceptedOnL2,
            transactions,
            outputs,
        }
    }
}
