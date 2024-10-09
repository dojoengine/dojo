/// Development configuration.
#[derive(Debug, Clone, Default)]
pub struct DevConfig {
    /// Whether to enable paying fees for transactions.
    ///
    /// If disabled, the transaction's sender will not be charged for the transaction. Any fee
    /// related checks will be skipped.
    ///
    /// For example, if the transaction's fee resources (ie max fee) is higher than the sender's
    /// balance, the transaction will still be considered valid.
    pub fee: bool,

    /// Whether to enable account validation when sending transaction.
    ///
    /// If disabled, the transaction's sender validation logic will not be executed in any
    /// circumstances. Sending a transaction with invalid signatures, will be considered valid.
    ///
    /// In the case where fee estimation or transaction simulation is done *WITHOUT* the
    /// `SKIP_VALIDATE` flag, if validation is disabled, then it would be as if the
    /// estimation/simulation was sent with `SKIP_VALIDATE`. Using `SKIP_VALIDATE` while
    /// validation is disabled is a no-op.
    pub account_validation: bool,
}
