use std::time::Duration;

use camino::Utf8PathBuf;
use dojo_test_utils::compiler::CompilerTestSetup;
use katana_runner::KatanaRunner;
use scarb::compiler::Profile;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{BlockId, BlockTag, Felt};

use super::{WorldContract, WorldContractReader};
use crate::manifest::{BaseManifest, OverlayManifest, BASE_DIR, MANIFESTS_DIR, OVERLAYS_DIR};
use crate::metadata::dojo_metadata_from_workspace;
use crate::migration::strategy::prepare_for_migration;
use crate::migration::world::WorldDiff;
use crate::migration::{Declarable, Deployable, TxnConfig};
use crate::utils::TransactionExt;

#[tokio::test(flavor = "multi_thread")]
async fn test_world_contract_reader() {
    let runner = KatanaRunner::new().expect("Fail to set runner");

    let setup = CompilerTestSetup::from_examples("../dojo-core", "../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let default_namespace = ws.current_package().unwrap().id.name.to_string();

    let manifest_dir = config.manifest_path().parent().unwrap();
    let target_dir = manifest_dir.join("target").join("dev");

    let mut account = runner.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let provider = account.provider();

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let dojo_metadata =
        dojo_metadata_from_workspace(&ws).expect("No current package with dojo metadata found.");

    let world_address = deploy_world(
        &runner,
        &manifest_dir.to_path_buf(),
        &target_dir.to_path_buf(),
        dojo_metadata.skip_migration,
        &default_namespace,
    )
    .await;

    let _world = WorldContractReader::new(world_address, provider);
}

pub async fn deploy_world(
    sequencer: &KatanaRunner,
    manifest_dir: &Utf8PathBuf,
    target_dir: &Utf8PathBuf,
    skip_migration: Option<Vec<String>>,
    default_namespace: &str,
) -> Felt {
    // Dev profile is used by default for testing:
    let profile_name = "dev";

    let mut manifest = BaseManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();

    if let Some(skip_manifests) = skip_migration {
        manifest.remove_tags(skip_manifests);
    }

    let overlay_dir = manifest_dir.join(OVERLAYS_DIR).join(profile_name);
    if overlay_dir.exists() {
        let overlay_manifest = OverlayManifest::load_from_path(&overlay_dir, &manifest).unwrap();
        manifest.merge(overlay_manifest);
    }

    let mut world = WorldDiff::compute(manifest.clone(), None);
    world.update_order(default_namespace).unwrap();

    let account = sequencer.account(0);

    let mut strategy =
        prepare_for_migration(None, Felt::from_hex("0x12345").unwrap(), target_dir, world).unwrap();
    strategy.resolve_variable(strategy.world_address().unwrap()).unwrap();

    let base_class_hash =
        strategy.base.unwrap().declare(&account, &TxnConfig::init_wait()).await.unwrap().class_hash;

    let world_address = strategy
        .world
        .unwrap()
        .deploy(
            manifest.clone().world.inner.class_hash,
            vec![base_class_hash],
            &account,
            &TxnConfig::init_wait(),
        )
        .await
        .unwrap()
        .contract_address;

    let mut declare_output = vec![];
    for model in strategy.models {
        let res = model.declare(&account, &TxnConfig::init_wait()).await.unwrap();
        declare_output.push(res);
    }

    let world = WorldContract::new(world_address, &account);

    world
        .register_namespace(&cainome::cairo_serde::ByteArray::from_string("dojo_examples").unwrap())
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    // Wondering why the `init_wait` is not enough and causes a nonce error.
    // May be to a delay to create the block as we are in instant mining.
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let calls = declare_output
        .iter()
        .map(|o| world.register_model_getcall(&o.class_hash.into()))
        .collect::<Vec<_>>();

    let _ = account.execute_v1(calls).send_with_cfg(&TxnConfig::init_wait()).await.unwrap();

    for contract in strategy.contracts {
        let declare_res = contract.declare(&account, &TxnConfig::default()).await.unwrap();
        contract
            .deploy_dojo_contract(
                world_address,
                declare_res.class_hash,
                base_class_hash,
                &account,
                &TxnConfig::init_wait(),
                &contract.diff.init_calldata,
            )
            .await
            .unwrap();
    }

    world_address
}
