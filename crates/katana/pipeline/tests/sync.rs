use std::future::IntoFuture;

use katana_pipeline::{stage, Pipeline};
use katana_provider::test_utils::test_provider;
use katana_provider::traits::block::{BlockNumberProvider, BlockProvider};
use katana_provider::traits::state::StateFactoryProvider;
use katana_provider::traits::state_update::StateUpdateProvider;
use starknet::providers::SequencerGatewayProvider;

#[tokio::test(flavor = "multi_thread")]
async fn fgw_sync() {
    let tip = 20;
    let chunk_size = 5;
    let db_provider = test_provider();

    // build stages

    let fgw = SequencerGatewayProvider::starknet_alpha_sepolia();
    let blocks = stage::Blocks::new(db_provider.clone(), fgw.clone(), 10);
    let classes = stage::Classes::new(db_provider.clone(), fgw, 10);

    let (mut pipeline, handle) = Pipeline::new(db_provider.clone(), chunk_size);
    pipeline.add_stage(blocks);
    pipeline.add_stage(classes);

    tokio::spawn(pipeline.into_future());
    handle.set_tip(tip / 2);
    handle.set_tip(tip);

    // check the db

    let latest_num = db_provider.latest_number().expect("failed to get latest block number");
    assert_eq!(latest_num, tip);

    for i in 0..latest_num {
        let block = db_provider.block(i.into()).expect("failed to get block");
        assert!(block.is_some());
    }

    let declared_classes = db_provider.declared_classes(latest_num.into()).unwrap().unwrap();
    let latest_state = db_provider.latest().expect("failed to get latest state");

    for class_hash in declared_classes.keys() {
        let class = latest_state.class(*class_hash).unwrap();
        assert!(class.is_some());
    }
}
