pub mod stateful;

use crate::tx::PoolTransaction;
use katana_executor::ExecutionError;
use std::marker::PhantomData;

pub trait Validator {
    type Tx: PoolTransaction;

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
#[derive(Debug)]
pub enum ValidationOutcome<T> {
    Valid(T),
    Invalid { tx: T, error: ExecutionError }, // aka rejected in starknet terms
}

/// A no-op validator that does nothing and assume all incoming transactions are valid.
#[derive(Debug)]
pub struct NoopValidator<T>(PhantomData<T>);

impl<T> Validator for NoopValidator<T> {
    type Tx = T;

    fn validate(
        &self,
        tx: Self::Tx,
    ) -> Result<ValidationOutcome<Self::Tx>, Box<dyn std::error::Error>> {
        Ok(ValidationOutcome::Valid(tx))
    }
}
