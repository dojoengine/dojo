use katana_runner::*;
use starknet::providers::Provider;

#[katana_test(2, false, "katana", "")]
async fn test_run() {
    for i in 0..10 {
        let logname = format!("katana-test_run-{}", i);
        let runner = KatanaRunner::new_with_name(&logname).expect("failed to start another katana");

        let _block_number = runner.provider().block_number().await.unwrap();
        // created by the macro at the beginning of the test
        let _other_block_number = runner.provider().block_number().await.unwrap();
    }
}

#[katana_test(2, true, "katana", "")]
async fn basic_macro_usage() {
    let _block_number = runner.provider().block_number().await.unwrap();
}
