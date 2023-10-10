mod utils;

use utils::snapbox::get_snapbox;
use utils::stdout::expected_stdout;
use std::fs;

#[test]
fn test_invalid_cairo_version() {
    let path = fs::canonicalize("./tests/data/invalid_cairo_version");
    let assert = get_snapbox()
        .arg("build")
        .current_dir(path.unwrap())
        .assert()
        .failure();
    assert.stdout_eq(expected_stdout("wrong-cairo-version"));
}
