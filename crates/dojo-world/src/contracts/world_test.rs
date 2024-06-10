use std::time::Duration;

use camino::Utf8PathBuf;
use dojo_test_utils::compiler;
use katana_runner::KatanaRunner;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{BlockId, BlockTag, FieldElement};

use super::{WorldContract, WorldContractReader};
use crate::manifest::{BaseManifest, OverlayManifest, BASE_DIR, MANIFESTS_DIR, OVERLAYS_DIR};
use crate::migration::strategy::prepare_for_migration;
use crate::migration::world::WorldDiff;
use crate::migration::{Declarable, Deployable, TxnConfig};

#[tokio::test(flavor = "multi_thread")]
async fn test_world_contract_reader() {
    let runner = KatanaRunner::new().expect("Fail to set runner");
    let config = compiler::copy_tmp_config(
        &Utf8PathBuf::from("../../examples/spawn-and-move"),
        &Utf8PathBuf::from("../dojo-core"),
    );

    let manifest_dir = config.manifest_path().parent().unwrap();
    let target_dir = manifest_dir.join("target").join("dev");

    let mut account = runner.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let provider = account.provider();

    let world_address =
        deploy_world(&runner, &manifest_dir.to_path_buf(), &target_dir.to_path_buf()).await;

    let _world = WorldContractReader::new(world_address, provider);
}

pub async fn deploy_world(
    sequencer: &KatanaRunner,
    manifest_dir: &Utf8PathBuf,
    target_dir: &Utf8PathBuf,
) -> FieldElement {
    // Dev profile is used by default for testing:
    let profile_name = "dev";

    let mut manifest = BaseManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();

    let overlay_manifest = OverlayManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(OVERLAYS_DIR),
    )
    .unwrap();

    manifest.merge(overlay_manifest);

    let mut world = WorldDiff::compute(manifest.clone(), None);
    world.update_order().unwrap();

    let account = sequencer.account(0);

    let mut strategy = prepare_for_migration(
        None,
        FieldElement::from_hex_be("0x12345").unwrap(),
        target_dir,
        world,
    )
    .unwrap();
    strategy.resolve_variable(strategy.world_address().unwrap()).unwrap();

    let base_class_hash =
        strategy.base.unwrap().declare(&account, &TxnConfig::default()).await.unwrap().class_hash;

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    let world_address = strategy
        .world
        .unwrap()
        .deploy(
            manifest.clone().world.inner.class_hash,
            vec![base_class_hash],
            &account,
            &TxnConfig::default(),
        )
        .await
        .unwrap()
        .contract_address;

    let mut declare_output = vec![];
    for model in strategy.models {
        let res = model.declare(&account, &TxnConfig::default()).await.unwrap();
        declare_output.push(res);
    }

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    let world = WorldContract::new(world_address, &account);

    let calls = declare_output
        .iter()
        .map(|o| world.register_model_getcall(&o.class_hash.into()))
        .collect::<Vec<_>>();

    let _ = account.execute(calls).send().await.unwrap();

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    for contract in strategy.contracts {
        let declare_res = contract.declare(&account, &TxnConfig::default()).await.unwrap();
        contract
            .deploy_dojo_contract(
                world_address,
                declare_res.class_hash,
                base_class_hash,
                &account,
                &TxnConfig::default(),
                &contract.diff.init_calldata,
            )
            .await
            .unwrap();
    }

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    world_address
}
