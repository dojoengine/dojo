use dojo_world::contracts::WorldContractReader;
use katana_runner::RunnerCtx;
use scarb_ui::Ui;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::Felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;

use crate::test_utils::setup;
use crate::{call, utils};

const CONTRACT_TAG: &str = "dojo_examples-actions";
const ENTRYPOINT: &str = "get_player_position";

#[tokio::test]
#[katana_runner::test(db_dir = "/tmp/spawn-and-move-db")]
async fn call_with_bad_address(sequencer: &RunnerCtx) {
    let world = setup::setup_with_world(sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    let ui = Ui::new(scarb_ui::Verbosity::Verbose, scarb_ui::OutputFormat::Text);

    assert!(
        call::call(
            &ui,
            world_reader,
            "0xBadCoffeeBadCode".to_string(),
            ENTRYPOINT.to_string(),
            vec![Felt::ZERO, Felt::ZERO],
            None
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[katana_runner::test(db_dir = "/tmp/spawn-and-move-db")]
async fn call_with_bad_name(sequencer: &RunnerCtx) {
    let world = setup::setup_with_world(sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    let ui = Ui::new(scarb_ui::Verbosity::Verbose, scarb_ui::OutputFormat::Text);

    assert!(
        call::call(
            &ui,
            world_reader,
            "BadName".to_string(),
            ENTRYPOINT.to_string(),
            vec![Felt::ZERO, Felt::ZERO],
            None
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[katana_runner::test(db_dir = "/tmp/spawn-and-move-db")]
async fn call_with_bad_entrypoint(sequencer: &RunnerCtx) {
    let world = setup::setup_with_world(sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    let ui = Ui::new(scarb_ui::Verbosity::Verbose, scarb_ui::OutputFormat::Text);

    assert!(
        call::call(
            &ui,
            world_reader,
            CONTRACT_TAG.to_string(),
            "BadEntryPoint".to_string(),
            vec![Felt::ZERO, Felt::ZERO],
            None
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[katana_runner::test(db_dir = "/tmp/spawn-and-move-db")]
async fn call_with_bad_calldata(sequencer: &RunnerCtx) {
    let world = setup::setup_with_world(sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    let ui = Ui::new(scarb_ui::Verbosity::Verbose, scarb_ui::OutputFormat::Text);

    assert!(
        call::call(
            &ui,
            world_reader,
            CONTRACT_TAG.to_string(),
            ENTRYPOINT.to_string(),
            vec![Felt::ZERO],
            None
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[katana_runner::test(db_dir = "/tmp/spawn-and-move-db")]
async fn call_with_contract_name(sequencer: &RunnerCtx) {
    let world = setup::setup_with_world(sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    let ui = Ui::new(scarb_ui::Verbosity::Verbose, scarb_ui::OutputFormat::Text);

    let r = call::call(
        &ui,
        world_reader,
        CONTRACT_TAG.to_string(),
        ENTRYPOINT.to_string(),
        vec![],
        None,
    )
    .await;

    assert!(r.is_ok());
}

#[tokio::test]
#[katana_runner::test(db_dir = "/tmp/spawn-and-move-db")]
async fn call_with_contract_address(sequencer: &RunnerCtx) {
    let ui = Ui::new(scarb_ui::Verbosity::Verbose, scarb_ui::OutputFormat::Text);

    let world = setup::setup_with_world(sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    let contract_address = utils::get_contract_address::<
        SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    >(&world, CONTRACT_TAG)
    .await
    .unwrap();

    assert!(
        call::call(
            &ui,
            world_reader,
            format!("{:#x}", contract_address),
            ENTRYPOINT.to_string(),
            vec![],
            None,
        )
        .await
        .is_ok()
    );
}
