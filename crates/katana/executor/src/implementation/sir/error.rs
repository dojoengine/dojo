use katana_primitives::FieldElement;
use sir::transaction::error::TransactionError;

use crate::ExecutionError;

use super::utils::{to_class_hash, to_felt};

impl From<TransactionError> for ExecutionError {
    fn from(error: TransactionError) -> Self {
        match error {
            TransactionError::ActualFeeExceedsMaxFee(actual_fee, max_fee) => {
                Self::ActualFeeExceedsMaxFee { max_fee, actual_fee }
            }

            TransactionError::ClassAlreadyDeclared(hash) => {
                Self::ClassAlreadyDeclared(to_class_hash(&hash))
            }

            TransactionError::InvalidTransactionNonce(expected, actual) => {
                let actual = FieldElement::from_dec_str(&actual).expect("must be valid");
                let expected = FieldElement::from_dec_str(&expected).expect("must be valid");
                Self::InvalidNonce { actual, expected }
            }

            TransactionError::MaxFeeExceedsBalance(max_fee, low, high) => {
                let balance_low = to_felt(&low);
                let balance_high = to_felt(&high);
                Self::InsufficientBalance { max_fee, balance_low, balance_high }
            }

            TransactionError::NotDeployedContract(address) => {
                let address = FieldElement::from_bytes_be(&address.0).expect("valid felt").into();
                Self::ContractNotDeployed(address)
            }

            TransactionError::MaxFeeTooLow(max_fee, min) => Self::MaxFeeTooLow { max_fee, min },
            TransactionError::EntryPointNotFound(selector) => Self::EntryPointNotFound(to_felt(&selector)),
            TransactionError::FeeTransferError(e) => Self::FeeTransferError(e.to_string()),
            e => Self::Other(e.to_string()),
        }
    }
}
