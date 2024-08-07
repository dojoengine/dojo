use crate::TxId;

pub trait Validator: Send + Sync {
    type Tx;

    fn validate(
        &self,
        tx: Self::Tx,
    ) -> Result<ValidationOutcome<Self::Tx>, Box<dyn std::error::Error>>;

    fn validate_all(
        &self,
        txs: Vec<Self::Tx>,
    ) -> Vec<Result<ValidationOutcome<Self::Tx>, Box<dyn std::error::Error>>> {
        txs.into_iter().map(|tx| self.validate(tx)).collect()
    }
}

// outcome of the validation phase. the variant of this enum determines on which pool
// the tx should be inserted into.
pub enum ValidationOutcome<T> {
    Valid { tx: T, dependent: Option<TxId> }, // valid and can be picked up by the ordering mechanism
    Invalid { tx: T, error: Box<dyn std::error::Error> }, // aka rejected in starknet terms
}

// Validates the incoming tx before adding them in the pool.
// Runs the validation logic of the tx and keep track of the tx dependencies.
pub struct StatefulValidator {}
