mod utils;

use clap::Parser;
use dojo_test_utils::compiler::build_test_config;
use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_world::metadata::dojo_metadata_from_workspace;
use scarb::ops;
use sozo::args::{Commands, SozoArgs};
use sozo::ops::migration::execute_strategy;
use sozo::ops::register;
use starknet::accounts::Account;

#[tokio::test(flavor = "multi_thread")]
async fn reregister_models() {
    let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let env_metadata = if config.manifest_path().exists() {
        let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
        dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
    } else {
        None
    };

    let migration = prepare_migration("../../examples/spawn-and-move/target/dev".into()).unwrap();

    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let account = sequencer.account();
    execute_strategy(&ws, &migration, &account, None).await.unwrap();
    let world_address = &format!("0x{:x}", &migration.world_address().unwrap());
    let account_address = &format!("0x{:x}", account.address());
    let moves_model_class_hash =
        "0x764906a97ff3e532e82b154908b25711cdec1c692bf68e3aba2a3dd9964a15c";
    let args_vec = [
        "sozo",
        "register",
        "model",
        moves_model_class_hash,
        "--world",
        world_address,
        "--account-address",
        account_address,
    ];
    let mut updated_env = env_metadata.unwrap();
    updated_env.rpc_url = Some(sequencer.url().to_string());

    let args = SozoArgs::parse_from(args_vec);
    match args.command {
        Commands::Register(args) => {
            register::execute(args.command, Some(updated_env.clone()), &config).await.unwrap();
        }
        _ => panic!("Expected \"sozo register model\" command!"),
    }

    sequencer.stop().unwrap();
}
