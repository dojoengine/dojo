use std::str::FromStr;

use dojo_test_utils::migration::copy_spawn_and_move_db;
use dojo_utils::TxnConfig;
use dojo_world::contracts::naming::compute_selector_from_tag;
use dojo_world::contracts::world::WorldContract;
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use scarb_ui::{OutputFormat, Ui, Verbosity};
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::Felt;

use crate::auth::{self, ResourceOwner, ResourceType, ResourceWriter};
use crate::execute;
use crate::test_utils::setup;

const ACTION_CONTRACT_NAME: &str = "dojo_examples-actions";
const DEFAULT_NAMESPACE: &str = "dojo_examples";
const MOVE_MODEL_TAG: &str = "dojo_examples-Moves";
const POSITION_MODEL_TAG: &str = "dojo_examples-Position";

fn get_resource_writers() -> [ResourceWriter; 3] {
    [
        ResourceWriter {
            resource: ResourceType::from_str(&format!("model:{MOVE_MODEL_TAG}")).unwrap(),
            tag_or_address: ACTION_CONTRACT_NAME.to_string(),
        },
        ResourceWriter {
            resource: ResourceType::from_str(&format!("model:{POSITION_MODEL_TAG}")).unwrap(),
            tag_or_address: ACTION_CONTRACT_NAME.to_string(),
        },
        ResourceWriter {
            resource: ResourceType::from_str(&format!("ns:{DEFAULT_NAMESPACE}")).unwrap(),
            tag_or_address: ACTION_CONTRACT_NAME.to_string(),
        },
    ]
}

fn get_resource_owners(owner: Felt) -> [ResourceOwner; 2] {
    [
        ResourceOwner {
            resource: ResourceType::from_str(&format!("model:{MOVE_MODEL_TAG}")).unwrap(),
            owner,
        },
        ResourceOwner {
            resource: ResourceType::from_str(&format!("model:{POSITION_MODEL_TAG}")).unwrap(),
            owner,
        },
    ]
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_grant_writer_ok() {
    let config = KatanaRunnerConfig { n_accounts: 10, ..Default::default() }
        .with_db_dir(copy_spawn_and_move_db().as_str());

    let sequencer = KatanaRunner::new_with_config(config).expect("Failed to start runner.");

    let world = setup::setup_with_world(&sequencer).await.unwrap();

    // Overlays already have the writer set up. But running again to ensure we don't
    // actually revert something with this call.
    auth::grant_writer(
        &Ui::new(Verbosity::Normal, OutputFormat::Text),
        &world,
        &get_resource_writers(),
        &TxnConfig { wait: true, ..Default::default() },
        DEFAULT_NAMESPACE,
        &None,
    )
    .await
    .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    assert!(execute_spawn(&world).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_revoke_writer_ok() {
    let config = KatanaRunnerConfig { n_accounts: 10, ..Default::default() }
        .with_db_dir(copy_spawn_and_move_db().as_str());

    let sequencer = KatanaRunner::new_with_config(config).expect("Failed to start runner.");

    let world = setup::setup_with_world(&sequencer).await.unwrap();

    auth::grant_writer(
        &Ui::new(Verbosity::Normal, OutputFormat::Text),
        &world,
        &get_resource_writers(),
        &TxnConfig { wait: true, ..Default::default() },
        DEFAULT_NAMESPACE,
        &None,
    )
    .await
    .unwrap();

    assert!(execute_spawn(&world).await);

    auth::revoke_writer(
        &Ui::new(Verbosity::Normal, OutputFormat::Text),
        &world,
        &get_resource_writers(),
        &TxnConfig { wait: true, ..Default::default() },
        DEFAULT_NAMESPACE,
        &None,
    )
    .await
    .unwrap();

    // Here it shouldn't be executable.
    assert!(!execute_spawn(&world).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_grant_owner_ok() {
    let move_model_selector = compute_selector_from_tag(MOVE_MODEL_TAG);
    let position_model_selector = compute_selector_from_tag(POSITION_MODEL_TAG);

    let config = KatanaRunnerConfig { n_accounts: 10, ..Default::default() }
        .with_db_dir(copy_spawn_and_move_db().as_str());

    let sequencer = KatanaRunner::new_with_config(config).expect("Failed to start runner.");
    println!("sequencer logs: {:?}", sequencer.log_file_path());

    let world = setup::setup_with_world(&sequencer).await.unwrap();

    let default_account = sequencer.account(0).address();
    let other_account = sequencer.account(1).address();

    assert!(world.is_owner(&move_model_selector, &default_account.into()).call().await.unwrap());
    assert!(
        world.is_owner(&position_model_selector, &default_account.into()).call().await.unwrap()
    );
    assert!(!world.is_owner(&move_model_selector, &other_account.into()).call().await.unwrap());
    assert!(!world.is_owner(&position_model_selector, &other_account.into()).call().await.unwrap());

    auth::grant_owner(
        &Ui::new(Verbosity::Normal, OutputFormat::Text),
        &world,
        &get_resource_owners(other_account),
        &TxnConfig { wait: true, ..Default::default() },
        DEFAULT_NAMESPACE,
        &None,
    )
    .await
    .unwrap();

    assert!(world.is_owner(&move_model_selector, &other_account.into()).call().await.unwrap());
    assert!(world.is_owner(&position_model_selector, &other_account.into()).call().await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_revoke_owner_ok() {
    let move_model_selector = compute_selector_from_tag(MOVE_MODEL_TAG);
    let position_model_selector = compute_selector_from_tag(POSITION_MODEL_TAG);

    let config = KatanaRunnerConfig { n_accounts: 10, ..Default::default() }
        .with_db_dir(copy_spawn_and_move_db().as_str());

    let sequencer = KatanaRunner::new_with_config(config).expect("Failed to start runner.");

    let world = setup::setup_with_world(&sequencer).await.unwrap();

    let default_account = sequencer.account(0).address();

    assert!(world.is_owner(&move_model_selector, &default_account.into()).call().await.unwrap());
    assert!(
        world.is_owner(&position_model_selector, &default_account.into()).call().await.unwrap()
    );

    auth::revoke_owner(
        &Ui::new(Verbosity::Normal, OutputFormat::Text),
        &world,
        &get_resource_owners(default_account),
        &TxnConfig { wait: true, ..Default::default() },
        DEFAULT_NAMESPACE,
        &None,
    )
    .await
    .unwrap();

    assert!(!world.is_owner(&move_model_selector, &default_account.into()).call().await.unwrap());
    assert!(
        !world.is_owner(&position_model_selector, &default_account.into()).call().await.unwrap()
    );
}

/// Executes the `spawn` system on `actions` contract.
///
/// # Returns
///
/// True if the execution was successful, false otherwise.
async fn execute_spawn<A: ConnectedAccount + Sync + Send + 'static>(
    world: &WorldContract<A>,
) -> bool {
    let contract_actions = ACTION_CONTRACT_NAME.to_string();
    let system_spawn = "spawn".to_string();

    let r = execute::execute(
        &Ui::new(Verbosity::Normal, OutputFormat::Text),
        contract_actions,
        system_spawn,
        vec![],
        world,
        &TxnConfig::init_wait(),
        &None,
    )
    .await;

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    r.is_ok()
}
