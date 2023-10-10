mod utils;

use utils::snapbox::get_snapbox;
use utils::stdout::expected_stdout;

#[test]
fn test_invalid_cairo_version() {
    let assert = get_snapbox()
        .arg("build")
        .current_dir("./tests/data/invalid_cairo_version")
        .assert()
        .failure();

    assert.stdout_eq(expected_stdout("wrong-cairo-version"));
}
