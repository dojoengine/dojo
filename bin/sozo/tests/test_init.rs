mod utils;

use utils::snapbox::get_snapbox;
use utils::stdout::expected_stdout;

#[test]
fn test_init() {
    let pt = assert_fs::TempDir::new().unwrap();

    let assert = get_snapbox().arg("init").current_dir(&pt).assert().success();

    let output = format!("{:#?}", assert.get_output());

    if output.contains("Couldn't find template for your current sozo version.") {
        assert.stdout_eq(expected_stdout("init_default"));
    } else {
        assert.stdout_eq(expected_stdout("init"));
    }
}
