mod utils;

use std::fs;

use assert_fs::fixture::PathChild;
use katana_runner::RunnerCtx;
use sozo_ops::account;
use starknet::accounts::Account;
use utils::snapbox::get_snapbox;

#[test]
fn test_account_new() {
    let pt = assert_fs::TempDir::new().unwrap();
    let dst_path = pt.child("keystore.json");
    let src_path = fs::canonicalize("./tests/test_data/keystore/keystore.json").unwrap();
    fs::copy(src_path, dst_path).unwrap();

    get_snapbox()
        .arg("account")
        .arg("new")
        .arg("account.json")
        .arg("--keystore")
        .arg("keystore.json")
        .arg("--password")
        .arg("password")
        .current_dir(&pt)
        .assert()
        .success();

    assert!(pt.child("account.json").exists());
}

#[tokio::test]
#[katana_runner::test(accounts = 1, fee = false)]
async fn test_account_fetch(runner: &RunnerCtx) {
    let pt = assert_fs::TempDir::new().unwrap();

    account::fetch(
        runner.owned_provider(),
        false,
        pt.child("account.json").to_path_buf(),
        runner.account(1).address(),
    )
    .await
    .unwrap();

    assert!(pt.child("account.json").exists());
}
