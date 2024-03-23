mod utils;

use std::{env, fs};

use assert_fs::fixture::PathChild;
use starknet_crypto::FieldElement;
use utils::snapbox::get_snapbox;

use sozo_ops::account::{self, FeeSetting};

use starknet::{
    accounts::Account,
    signers::{LocalWallet, SigningKey},
};

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

#[katana_runner::katana_test(1, true)]
async fn test_account_deploy() {
    let account_path = fs::canonicalize("./tests/test_data/account/account.json").unwrap();
    let keystore_path = fs::canonicalize("./tests/test_data/keystore/keystore.json").unwrap();

    let pt = assert_fs::TempDir::new().unwrap();
    fs::copy(account_path.clone(), pt.child("account.json")).unwrap();

    let signer = LocalWallet::from_signing_key(
        SigningKey::from_keystore(keystore_path, "password").unwrap(),
    );

    let contents = fs::read_to_string(pt.child("account.json")).unwrap();
    assert!(contents.contains(r#""status": "undeployed""#));

    env::set_var("PASS_STDIN", "1");
    account::deploy(
        runner.owned_provider(),
        signer,
        FeeSetting::Manual(FieldElement::from(1000_u32)),
        false,
        None,
        1000,
        account_path,
    )
    .await
    .unwrap();

    let contents = fs::read_to_string(pt.child("account.json")).unwrap();
    assert!(contents.contains(r#""status": "deployed""#));
}

#[katana_runner::katana_test(1, true)]
async fn test_account_fetch() {
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
