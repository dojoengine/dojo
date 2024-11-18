use katana_pipeline::{stage, Pipeline};
use katana_provider::test_utils::test_provider;
use katana_provider::traits::block::BlockNumberProvider;
use katana_provider::traits::state::StateFactoryProvider;
use katana_provider::traits::state_update::StateUpdateProvider;
use starknet::providers::SequencerGatewayProvider;

#[tokio::test]
async fn fgw_sync() {
    let tip = 10;
    let chunk_size = 5;
    let db_provider = test_provider();

    // build stages

    let fgw = SequencerGatewayProvider::starknet_alpha_sepolia();
    let blocks = stage::Blocks::new(db_provider.clone(), fgw.clone());
    let classes = stage::Classes::new(db_provider.clone(), fgw);

    let mut pipeline = Pipeline::new(tip, &db_provider, chunk_size);
    pipeline.add_stage(blocks);
    pipeline.add_stage(classes);

    pipeline.run().await.expect("failed to run pipelien");

    // check the db

    let latest_num = db_provider.latest_number().expect("failed to get latest block number");
    assert_eq!(latest_num, tip);

    let declared_classes = db_provider.declared_classes(latest_num.into()).unwrap().unwrap();
    let latest_state = db_provider.latest().expect("failed to get latest state");

    for class_hash in declared_classes.keys() {
        let class = latest_state.class(*class_hash).unwrap();
        assert!(class.is_some());
    }
}
