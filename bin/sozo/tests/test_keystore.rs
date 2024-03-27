mod utils;

use std::fs;

use assert_fs::fixture::{FileTouch, PathChild};
use utils::snapbox::get_snapbox;

#[test]
fn test_keystore_new() {
    let pt = assert_fs::TempDir::new().unwrap();

    get_snapbox()
        .arg("keystore")
        .arg("new")
        .arg("keystore.json")
        .arg("--password")
        .arg("password")
        .current_dir(&pt)
        .assert()
        .success();

    assert!(pt.child("keystore.json").exists());
}

#[test]
fn test_keystore_new_force() {
    let pt = assert_fs::TempDir::new().unwrap();

    pt.child("keystore.json").touch().unwrap();

    get_snapbox()
        .arg("keystore")
        .arg("new")
        .arg("--password")
        .arg("password")
        .arg("keystore.json")
        .arg("--force")
        .current_dir(&pt)
        .assert()
        .success();

    assert!(pt.child("keystore.json").exists());

    let contents = fs::read_to_string(pt.child("keystore.json")).unwrap();
    assert!(!contents.is_empty());
}

#[test]
fn test_keystore_from_key() {
    let pt = assert_fs::TempDir::new().unwrap();

    get_snapbox()
        .arg("keystore")
        .arg("from-key")
        .arg("keystore.json")
        .arg("--password")
        .arg("password")
        .arg("--private-key")
        .arg("0x123")
        .current_dir(&pt)
        .assert()
        .success();

    assert!(pt.child("keystore.json").exists());
}

#[test]
fn test_keystore_inspect() {
    let path = fs::canonicalize("./tests/test_data/keystore").unwrap();

    let assert = get_snapbox()
        .arg("keystore")
        .arg("inspect")
        .arg("keystore.json")
        .arg("--password")
        .arg("password")
        .current_dir(path)
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    assert_eq!(
        output.trim(),
        "Public key: 0x0566d69d8c99f62bc71118399bab25c1f03719463eab8d6a444cd11ece131616"
    )
}

#[test]
fn test_keystore_inspect_raw() {
    let path = fs::canonicalize("./tests/test_data/keystore").unwrap();

    let assert = get_snapbox()
        .arg("keystore")
        .arg("inspect")
        .arg("keystore.json")
        .arg("--password")
        .arg("password")
        .arg("--raw")
        .current_dir(path)
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    assert_eq!(output.trim(), "0x0566d69d8c99f62bc71118399bab25c1f03719463eab8d6a444cd11ece131616")
}

#[test]
fn test_keystore_inspect_private() {
    let path = fs::canonicalize("./tests/test_data/keystore").unwrap();

    let assert = get_snapbox()
        .arg("keystore")
        .arg("inspect-private")
        .arg("keystore.json")
        .arg("--password")
        .arg("password")
        .current_dir(path)
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    assert_eq!(
        output.trim(),
        "Private key: 0x0000000000000000000000000000000000000000000000000000000000000123"
    )
}
