use blockifier::blockifier::stateful_validator;
use katana_executor::StateProviderDb;
use parking_lot::Mutex;

use super::{ValidationOutcome, Validator};

// Validates the incoming tx before adding them in the pool.
// Runs the validation logic of the tx and keep track of the tx dependencies.
//
// the executor must wrap the pending state so that it can perform validation against the pending block state.
// this validator only validate the tx validation and doesn't take into consideration of its nonce.
pub struct StatefulValidator {
    inner: Mutex<stateful_validator::StatefulValidator<StateProviderDb<'static>>>,
}

impl Validator for StatefulValidator {
    type Tx = ();

    fn validate(
        &self,
        tx: Self::Tx,
    ) -> Result<ValidationOutcome<Self::Tx>, Box<dyn std::error::Error>> {
        let result = self.inner.lock().perform_validations(todo!(), None);
        Ok(ValidationOutcome::Valid { tx })
    }
}