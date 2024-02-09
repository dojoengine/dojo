use katana_primitives::transaction::TxWithHash;
use snos::io::InternalTransaction;

pub fn snos_internal_from_tx(_tx_with_hash: &TxWithHash) -> InternalTransaction {
    let internal = InternalTransaction::default();

    // match tx_with_hash.transaction {
    //     Tx::Invoke(tx) => {
    //         internal.hash_value = tx_with_hash.hash;
    //     },
    //     _ => {}
    // };

    internal
}
