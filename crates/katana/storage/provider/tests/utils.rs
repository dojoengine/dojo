use katana_primitives::block::{Block, BlockHash, FinalityStatus, Header, SealedBlockWithStatus};
use katana_primitives::receipt::{InvokeTxReceipt, Receipt};
use katana_primitives::transaction::{Tx, TxHash, TxWithHash};
use katana_primitives::FieldElement;

pub fn generate_dummy_txs_and_receipts(count: usize) -> (Vec<TxWithHash>, Vec<Receipt>) {
    let mut txs = Vec::with_capacity(count);
    let mut receipts = Vec::with_capacity(count);

    // TODO: generate random txs and receipts variants
    for _ in 0..count {
        txs.push(TxWithHash {
            hash: TxHash::from(rand::random::<u128>()),
            transaction: Tx::Invoke(Default::default()),
        });

        receipts.push(Receipt::Invoke(InvokeTxReceipt::default()));
    }

    (txs, receipts)
}

pub fn generate_dummy_blocks_and_receipts(
    count: u64,
) -> Vec<(SealedBlockWithStatus, Vec<Receipt>)> {
    let mut blocks = Vec::with_capacity(count as usize);
    let mut parent_hash: BlockHash = 0u8.into();

    for i in 0..count {
        let tx_count = (rand::random::<u64>() % 10) as usize;
        let (body, receipts) = generate_dummy_txs_and_receipts(tx_count);

        let header = Header { parent_hash, number: i, ..Default::default() };
        let block =
            Block { header, body }.seal_with_hash(FieldElement::from(rand::random::<u128>()));

        parent_hash = block.header.hash;

        blocks.push((
            SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL2 },
            receipts,
        ));
    }

    blocks
}
