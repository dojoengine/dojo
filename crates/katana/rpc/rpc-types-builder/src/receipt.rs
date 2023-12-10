use katana_primitives::transaction::TxHash;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider,
};
use katana_rpc_types::receipt::TxReceipt;

/// A builder for building RPC transaction receipt types.
pub struct ReceiptBuilder<P> {
    provider: P,
    transaction_hash: TxHash,
}

impl<P> ReceiptBuilder<P> {
    pub fn new(transaction_hash: TxHash, provider: P) -> Self {
        Self { provider, transaction_hash }
    }
}

impl<P> ReceiptBuilder<P>
where
    P: TransactionProvider + TransactionStatusProvider + ReceiptProvider,
{
    pub fn build(&self) -> anyhow::Result<Option<TxReceipt>> {
        let receipt = ReceiptProvider::receipt_by_hash(&self.provider, self.transaction_hash)?;
        let Some(receipt) = receipt else { return Ok(None) };

        let transaction_hash = self.transaction_hash;
        let (block_number, block_hash) = TransactionProvider::transaction_block_num_and_hash(
            &self.provider,
            self.transaction_hash,
        )?
        .expect("must exist");

        let finality_status =
            TransactionStatusProvider::transaction_status(&self.provider, self.transaction_hash)?
                .expect("must exist");

        Ok(Some(TxReceipt::new(
            transaction_hash,
            block_number,
            block_hash,
            finality_status,
            receipt,
        )))
    }
}
