use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use cainome::cairo_serde::{ByteArray, CairoSerde, ContractAddress};
use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::copy_spawn_and_move_db;
use dojo_utils::{TransactionExt, TransactionWaiter, TxnConfig};
use dojo_world::contracts::naming::{compute_bytearray_hash, compute_selector_from_names};
use dojo_world::contracts::world::{WorldContract, WorldContractReader};
use katana_runner::RunnerCtx;
use num_traits::ToPrimitive;
use scarb::compiler::Profile;
use sozo_scarbext::WorkspaceExt;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use starknet::accounts::Account;
use starknet::core::types::{Call, Felt, U256};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet_crypto::poseidon_hash_many;
use tempfile::NamedTempFile;
use tokio::sync::broadcast;
use torii_sqlite::cache::ModelCache;
use torii_sqlite::executor::Executor;
use torii_sqlite::types::{Contract, ContractType, Token};
use torii_sqlite::utils::u256_to_sql_string;
use torii_sqlite::Sql;

use crate::engine::{Engine, EngineConfig, Processors};

pub async fn bootstrap_engine<P>(
    world: WorldContractReader<P>,
    mut db: Sql,
    provider: P,
    contracts: &[Contract],
) -> Result<Engine<P>, Box<dyn std::error::Error>>
where
    P: Provider + Send + Sync + core::fmt::Debug + Clone + 'static,
{
    let (shutdown_tx, _) = broadcast::channel(1);
    let to = provider.block_hash_and_number().await?.block_number;
    let mut engine = Engine::new(
        world,
        db.clone(),
        provider,
        Processors { ..Processors::default() },
        EngineConfig::default(),
        shutdown_tx,
        None,
        contracts,
    );

    let data = engine.fetch_range(0, to, &HashMap::new(), to).await.unwrap();
    engine.process_range(data).await.unwrap();

    db.flush().await.unwrap();
    db.apply_cache_diff().await.unwrap();
    db.execute().await.unwrap();

    Ok(engine)
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_load_from_remote(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();

    let actions_address = world_local
        .get_contract_address_local(compute_selector_from_names("ns", "actions"))
        .unwrap();

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(actions_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // spawn
    let tx = &account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();

    // move
    let tx = &account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("move").unwrap(),
            calldata: vec![Felt::ONE],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let contracts = vec![Contract { address: world_reader.address, r#type: ContractType::WORLD }];
    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(pool.clone(), sender.clone(), &contracts, model_cache.clone()).await.unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider, &contracts).await.unwrap();

    let _block_timestamp = 1710754478_u64;
    let models = sqlx::query("SELECT * FROM models").fetch_all(&pool).await.unwrap();
    assert_eq!(models.len(), 10);

    let (id, name, namespace, packed_size, unpacked_size): (String, String, String, u8, u8) =
        sqlx::query_as(
            "SELECT id, name, namespace, packed_size, unpacked_size FROM models WHERE name = \
             'Position'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(id, format!("{:#x}", compute_selector_from_names("ns", "Position")));
    assert_eq!(name, "Position");
    assert_eq!(namespace, "ns");
    assert_eq!(packed_size, 1);
    assert_eq!(unpacked_size, 2);

    let (id, name, namespace, packed_size, unpacked_size): (String, String, String, u8, u8) =
        sqlx::query_as(
            "SELECT id, name, namespace, packed_size, unpacked_size FROM models WHERE name = \
             'Moves'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(id, format!("{:#x}", compute_selector_from_names("ns", "Moves")));
    assert_eq!(name, "Moves");
    assert_eq!(namespace, "ns");
    assert_eq!(packed_size, 0);
    assert_eq!(unpacked_size, 2);

    let (id, name, namespace, packed_size, unpacked_size): (String, String, String, u8, u8) =
        sqlx::query_as(
            "SELECT id, name, namespace, packed_size, unpacked_size FROM models WHERE name = \
             'PlayerConfig'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(id, format!("{:#x}", compute_selector_from_names("ns", "PlayerConfig")));
    assert_eq!(name, "PlayerConfig");
    assert_eq!(namespace, "ns");
    assert_eq!(packed_size, 0);
    assert_eq!(unpacked_size, 0);

    assert_eq!(count_table("entities", &pool).await, 2);
    assert_eq!(count_table("event_messages", &pool).await, 2);

    let (id, keys): (String, String) = sqlx::query_as(
        format!(
            "SELECT id, keys FROM entities WHERE id = '{:#x}'",
            poseidon_hash_many(&[account.address()])
        )
        .as_str(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(id, format!("{:#x}", poseidon_hash_many(&[account.address()])));
    assert_eq!(keys, format!("{:#x}/", account.address()));
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_load_from_remote_erc20(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let actions_address = world_local
        .external_contracts
        .iter()
        .find(|c| c.instance_name == "WoodToken")
        .unwrap()
        .address;

    let mut balance = U256::from(0u64);

    // mint 123456789 wei tokens
    let tx = &account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("mint").unwrap(),
            calldata: vec![Felt::from(123456789), Felt::ZERO],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();
    balance += U256::from(123456789u32);

    // transfer 12345 tokens to some other address
    let tx = &account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("transfer").unwrap(),
            calldata: vec![Felt::ONE, Felt::from(12345), Felt::ZERO],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();
    balance -= U256::from(12345u32);

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let contracts = vec![Contract { address: actions_address, r#type: ContractType::ERC20 }];

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(pool.clone(), sender.clone(), &contracts, model_cache.clone()).await.unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider, &contracts).await.unwrap();

    // first check if we indexed the token
    let token = sqlx::query_as::<_, Token>(
        format!("SELECT * from tokens where contract_address = '{:#x}'", actions_address).as_str(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(token.name, "Wood");
    assert_eq!(token.symbol, "WOOD");
    assert_eq!(token.decimals, 18);

    // check the balance
    let remote_balance = sqlx::query_scalar::<_, String>(
        format!(
            "SELECT balance FROM token_balances WHERE account_address = '{:#x}' AND \
             contract_address = '{:#x}'",
            account.address(),
            actions_address
        )
        .as_str(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let remote_balance = crypto_bigint::U256::from_be_hex(remote_balance.trim_start_matches("0x"));
    assert_eq!(balance, remote_balance.into());
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_load_from_remote_erc721(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let badge_address =
        world_local.external_contracts.iter().find(|c| c.instance_name == "Badge").unwrap().address;

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(badge_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // Mint multiple NFTs with different IDs
    for token_id in 1..=5 {
        let tx = &account
            .execute_v3(vec![Call {
                to: badge_address,
                selector: get_selector_from_name("mint").unwrap(),
                calldata: vec![Felt::from(token_id), Felt::ZERO],
            }])
            .send()
            .await
            .unwrap();

        TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();
    }

    // Transfer NFT ID 1 and 2 to another address
    for token_id in 1..=2 {
        let tx = &account
            .execute_v3(vec![Call {
                to: badge_address,
                selector: get_selector_from_name("transfer_from").unwrap(),
                calldata: vec![account.address(), Felt::ONE, Felt::from(token_id), Felt::ZERO],
            }])
            .send()
            .await
            .unwrap();

        TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();
    }

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let contracts = vec![Contract { address: badge_address, r#type: ContractType::ERC721 }];
    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(pool.clone(), sender.clone(), &contracts, model_cache.clone()).await.unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider, &contracts).await.unwrap();

    // Check if we indexed all tokens
    let tokens = sqlx::query_as::<_, Token>(
        format!(
            "SELECT * from tokens where contract_address = '{:#x}' ORDER BY token_id",
            badge_address
        )
        .as_str(),
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(tokens.len(), 5, "Should have indexed 5 different tokens");

    for (i, token) in tokens.iter().enumerate() {
        assert_eq!(token.name, "Badge");
        assert_eq!(token.symbol, "BDG");
        assert_eq!(token.decimals, 0);
        let token_id = crypto_bigint::U256::from_be_hex(token.token_id.trim_start_matches("0x"));
        assert_eq!(
            U256::from(token_id),
            U256::from(i.to_u32().unwrap() + 1),
            "Token IDs should be sequential"
        );
    }

    // Check balances for transferred tokens
    for token_id in 1..=2 {
        let balance = sqlx::query_scalar::<_, String>(
            format!(
                "SELECT balance FROM token_balances WHERE account_address = '{:#x}' AND \
                 contract_address = '{:#x}' AND token_id = '{:#x}:{}'",
                Felt::ONE,
                badge_address,
                badge_address,
                u256_to_sql_string(&U256::from(token_id as u32))
            )
            .as_str(),
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let balance = crypto_bigint::U256::from_be_hex(balance.trim_start_matches("0x"));
        assert_eq!(
            U256::from(balance),
            U256::from(1u8),
            "Sender should have balance of 1 for transferred tokens"
        );
    }

    // Check balances for non-transferred tokens
    for token_id in 3..=5 {
        let balance = sqlx::query_scalar::<_, String>(
            format!(
                "SELECT balance FROM token_balances WHERE account_address = '{:#x}' AND \
                 contract_address = '{:#x}' AND token_id = '{:#x}:{}'",
                account.address(),
                badge_address,
                badge_address,
                u256_to_sql_string(&U256::from(token_id as u32))
            )
            .as_str(),
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let balance = crypto_bigint::U256::from_be_hex(balance.trim_start_matches("0x"));
        assert_eq!(
            U256::from(balance),
            U256::from(1u8),
            "Original owner should have balance of 1 for non-transferred tokens"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_load_from_remote_erc1155(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let other_account = sequencer.account(1);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let rewards_address = world_local
        .external_contracts
        .iter()
        .find(|c| c.instance_name == "Rewards")
        .unwrap()
        .address;

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(rewards_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // Mint different amounts for different token IDs
    let token_amounts: Vec<(u32, u32)> = vec![
        (1, 100),  // Token ID 1, amount 100
        (2, 500),  // Token ID 2, amount 500
        (3, 1000), // Token ID 3, amount 1000
    ];

    for (token_id, amount) in &token_amounts {
        let tx = &account
            .execute_v3(vec![Call {
                to: rewards_address,
                selector: get_selector_from_name("mint").unwrap(),
                calldata: vec![Felt::from(*token_id), Felt::ZERO, Felt::from(*amount), Felt::ZERO],
            }])
            .send()
            .await
            .unwrap();

        TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();
    }

    // Transfer half of each token amount to another address
    for (token_id, amount) in &token_amounts {
        let tx = &account
            .execute_v3(vec![Call {
                to: rewards_address,
                selector: get_selector_from_name("transfer_from").unwrap(),
                calldata: vec![
                    account.address(),
                    other_account.address(),
                    Felt::from(*token_id),
                    Felt::ZERO,
                    Felt::from(amount / 2),
                    Felt::ZERO,
                ],
            }])
            .send()
            .await
            .unwrap();

        TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();
    }

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let contracts = vec![Contract { address: rewards_address, r#type: ContractType::ERC1155 }];
    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(pool.clone(), sender.clone(), &contracts, model_cache.clone()).await.unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider, &contracts).await.unwrap();

    // Check if we indexed all tokens
    let tokens = sqlx::query_as::<_, Token>(
        format!(
            "SELECT * from tokens where contract_address = '{:#x}' ORDER BY token_id",
            rewards_address
        )
        .as_str(),
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(tokens.len(), token_amounts.len(), "Should have indexed all token types");

    for token in &tokens {
        assert_eq!(token.name, "");
        assert_eq!(token.symbol, "");
        assert_eq!(token.decimals, 0);
    }

    // Check balances for all tokens
    for (token_id, original_amount) in token_amounts {
        // Check recipient balance
        let recipient_balance = sqlx::query_scalar::<_, String>(
            format!(
                "SELECT balance FROM token_balances WHERE account_address = '{:#x}' AND \
                 contract_address = '{:#x}' AND token_id = '{:#x}:{}'",
                other_account.address(),
                rewards_address,
                rewards_address,
                u256_to_sql_string(&U256::from(token_id))
            )
            .as_str(),
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let recipient_balance =
            crypto_bigint::U256::from_be_hex(recipient_balance.trim_start_matches("0x"));
        assert_eq!(
            U256::from(recipient_balance),
            U256::from(original_amount / 2),
            "Recipient should have half of original amount for token {}",
            token_id
        );

        // Check sender remaining balance
        let sender_balance = sqlx::query_scalar::<_, String>(
            format!(
                "SELECT balance FROM token_balances WHERE account_address = '{:#x}' AND \
                 contract_address = '{:#x}' AND token_id = '{:#x}:{}'",
                account.address(),
                rewards_address,
                rewards_address,
                u256_to_sql_string(&U256::from(token_id))
            )
            .as_str(),
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let sender_balance =
            crypto_bigint::U256::from_be_hex(sender_balance.trim_start_matches("0x"));
        assert_eq!(
            U256::from(sender_balance),
            U256::from(original_amount / 2),
            "Sender should have half of original amount for token {}",
            token_id
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_load_from_remote_del(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();
    let actions_address = world_local
        .get_contract_address_local(compute_selector_from_names("ns", "actions"))
        .unwrap();

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(actions_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // spawn
    let res = account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // Set player config.
    let res = account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("set_player_config").unwrap(),
            // Empty ByteArray.
            calldata: vec![Felt::ZERO, Felt::ZERO, Felt::ZERO],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    let res = account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("reset_player_config").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let contracts = vec![Contract { address: world_reader.address, r#type: ContractType::WORLD }];
    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(pool.clone(), sender.clone(), &contracts, model_cache.clone()).await.unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider, &contracts).await.unwrap();

    assert_eq!(count_table("ns-PlayerConfig", &pool).await, 0);
    assert_eq!(count_table("ns-Position", &pool).await, 0);
    assert_eq!(count_table("ns-Moves", &pool).await, 0);

    // our entity model relations should be deleted for our player entity
    let entity_model_count: i64 = sqlx::query_scalar(
        format!(
            "SELECT COUNT(*) FROM entity_model WHERE entity_id = '{:#x}'",
            poseidon_hash_many(&[account.address()])
        )
        .as_str(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(entity_model_count, 0);
    // our player entity should be deleted
    let entity_count: i64 = sqlx::query_scalar(
        format!(
            "SELECT COUNT(*) FROM entities WHERE id = '{:#x}'",
            poseidon_hash_many(&[account.address()])
        )
        .as_str(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(entity_count, 0);

    // TODO: check how we can have a test that is more chronological with Torii re-syncing
    // to ensure we can test intermediate states.
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_update_with_set_record(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();
    let actions_address = world_local
        .get_contract_address_local(compute_selector_from_names("ns", "actions"))
        .unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(actions_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // Send spawn transaction
    let spawn_res = account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(spawn_res.transaction_hash, &provider).await.unwrap();

    // Send move transaction
    let move_res = account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("move").unwrap(),
            calldata: vec![Felt::ZERO],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(move_res.transaction_hash, &provider).await.unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);

    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let contracts = vec![Contract { address: world_reader.address, r#type: ContractType::WORLD }];
    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(pool.clone(), sender.clone(), &contracts, model_cache.clone()).await.unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider, &contracts).await.unwrap();
}

#[ignore = "This test is being flaky and need to find why. Sometimes it fails, sometimes it passes."]
#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_load_from_remote_update(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();
    let actions_address = world_local
        .get_contract_address_local(compute_selector_from_names("ns", "actions"))
        .unwrap();

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(actions_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // spawn
    let res = account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // Set player config.
    let res = account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("set_player_config").unwrap(),
            // Empty ByteArray.
            calldata: vec![Felt::ZERO, Felt::ZERO, Felt::ZERO],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    let name = ByteArray::from_string("mimi").unwrap();
    let res = account
        .execute_v3(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("update_player_config_name").unwrap(),
            calldata: ByteArray::cairo_serialize(&name),
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let contracts = vec![Contract { address: world_reader.address, r#type: ContractType::WORLD }];
    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(pool.clone(), sender.clone(), &contracts, model_cache.clone()).await.unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider, &contracts).await.unwrap();

    let name: String = sqlx::query_scalar(
        format!(
            "SELECT name FROM [ns-PlayerConfig] WHERE internal_id = '{:#x}'",
            poseidon_hash_many(&[account.address()])
        )
        .as_str(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(name, "mimi");
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_update_token_metadata_erc1155(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let rewards_address = world_local
        .external_contracts
        .iter()
        .find(|c| c.instance_name == "Rewards")
        .unwrap()
        .address;

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(rewards_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    let tx = &account
        .execute_v3(vec![Call {
            to: rewards_address,
            selector: get_selector_from_name("mint").unwrap(),
            calldata: vec![Felt::from(1), Felt::ZERO, Felt::from(1), Felt::ZERO],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();

    let owner_account = sequencer.account(3);
    let tx = &owner_account
        .execute_v3(vec![Call {
            to: rewards_address,
            selector: get_selector_from_name("update_token_metadata").unwrap(),
            calldata: vec![Felt::from(1), Felt::ZERO],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();

    let block_number = provider.block_number().await.unwrap();

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let contracts = vec![Contract { address: rewards_address, r#type: ContractType::ERC1155 }];
    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(pool.clone(), sender.clone(), &contracts, model_cache.clone()).await.unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), Arc::clone(&provider), &contracts)
        .await
        .unwrap();

    let token = sqlx::query_as::<_, Token>(
        format!(
            "SELECT * from tokens where contract_address = '{:#x}' ORDER BY token_id",
            rewards_address
        )
        .as_str(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(token.metadata.contains(&format!(
        "https://api.dicebear.com/9.x/lorelei-neutral/png?seed={}",
        block_number + 1
    )));
}

/// Count the number of rows in a table.
///
/// # Arguments
/// * `table_name` - The name of the table to count the rows of.
/// * `pool` - The database pool.
///
/// # Returns
/// The number of rows in the table.
async fn count_table(table_name: &str, pool: &sqlx::Pool<sqlx::Sqlite>) -> i64 {
    let count_query = format!("SELECT COUNT(*) FROM [{}]", table_name);
    let count: (i64,) = sqlx::query_as(&count_query).fetch_one(pool).await.unwrap();

    count.0
}
