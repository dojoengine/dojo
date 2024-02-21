mod fixtures;

use fixtures::blockifier::factory as blockifier_factory;
use fixtures::{state_provider, valid_blocks};
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_executor::ExecutorFactory;
use katana_primitives::block::ExecutableBlock;
use katana_primitives::transaction::TxWithHash;
use katana_provider::traits::state::StateProvider;

#[rstest::rstest]
fn test_blockifier_executor_with_valid_blocks(
    #[from(blockifier_factory)] factory: BlockifierFactory,
    #[from(state_provider)] state: Box<dyn StateProvider>,
    #[from(valid_blocks)] blocks: [ExecutableBlock; 3],
) {
    let mut executor = factory.with_state(state);

    for block in blocks {
        executor.execute_block(block.clone()).unwrap();

        let expected_txs: Vec<TxWithHash> = block.body.iter().map(|t| t.into()).collect();
        let actual_txs = executor.transactions().iter().map(|(t, _)| t.clone()).collect::<Vec<_>>();

        assert_eq!(
            actual_txs, expected_txs,
            "all transactions should have been executed and stored"
        );
        assert!(
            executor.transactions().iter().all(|(_, rct)| rct.is_some()),
            "all transactions should have receipts"
        );
    }
}
