use std::str::FromStr;

use dojo_world::contracts::naming::compute_selector_from_tag;
use dojo_world::contracts::world::WorldContract;
use dojo_world::migration::TxnConfig;
use katana_runner::KatanaRunner;
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

fn get_resource_writers() -> [ResourceWriter; 2] {
    [
        ResourceWriter {
            resource: ResourceType::from_str(&format!("model:{MOVE_MODEL_TAG}")).unwrap(),
            tag_or_address: ACTION_CONTRACT_NAME.to_string(),
        },
        ResourceWriter {
            resource: ResourceType::from_str(&format!("model:{POSITION_MODEL_TAG}")).unwrap(),
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
    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let world = setup::setup(&sequencer).await.unwrap();

    // as writer roles are setup by default in setup::setup, this should work
    assert!(execute_spawn(&world).await);

    // remove writer roles
    auth::revoke_writer(
        &Ui::new(Verbosity::Normal, OutputFormat::Text),
        &world,
        &get_resource_writers(),
        TxnConfig { wait: true, ..Default::default() },
        DEFAULT_NAMESPACE,
    )
    .await
    .unwrap();

    // without writer roles, this should fail
    assert!(!execute_spawn(&world).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_revoke_writer_ok() {
    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let world = setup::setup(&sequencer).await.unwrap();

    assert!(execute_spawn(&world).await);

    // Here we are revoking the access again.
    auth::revoke_writer(
        &Ui::new(Verbosity::Normal, OutputFormat::Text),
        &world,
        &get_resource_writers(),
        TxnConfig { wait: true, ..Default::default() },
        DEFAULT_NAMESPACE,
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

    let sequencer = KatanaRunner::new().expect("Failed to start runner.");
    let world = setup::setup(&sequencer).await.unwrap();

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
        TxnConfig { wait: true, ..Default::default() },
        DEFAULT_NAMESPACE,
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

    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let world = setup::setup(&sequencer).await.unwrap();

    let default_account = sequencer.account(0).address();

    assert!(world.is_owner(&move_model_selector, &default_account.into()).call().await.unwrap());
    assert!(
        world.is_owner(&position_model_selector, &default_account.into()).call().await.unwrap()
    );

    auth::revoke_owner(
        &Ui::new(Verbosity::Normal, OutputFormat::Text),
        &world,
        &get_resource_owners(default_account),
        TxnConfig { wait: true, ..Default::default() },
        DEFAULT_NAMESPACE,
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
    )
    .await;

    println!("ERR {:?}", r);

    r.is_ok()
}
