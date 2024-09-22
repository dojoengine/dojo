use starknet::providers::Provider;

#[katana_runner::test(fee = false, accounts = 10)]
fn simple(runner: &RunnerCtx) {
    assert_eq!(runner.accounts().len(), 10);
}

#[katana_runner::test]
fn with_return(_: &RunnerCtx) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[tokio::test]
#[katana_runner::test]
async fn with_async(ctx: &RunnerCtx) -> Result<(), Box<dyn std::error::Error>> {
    let provider = ctx.provider();
    let _ = provider.chain_id().await?;
    Ok(())
}

// #[katana_test(2, false)]
// async fn test_run() {
//     for i in 0..10 {
//         let logname = format!("katana-test_run-{}", i);
//         let runner = KatanaRunner::new_with_config(KatanaRunnerConfig {
//             run_name: Some(logname),
//             ..Default::default()
//         })
//         .expect("failed to start another katana");

//         let _block_number = runner.provider().block_number().await.unwrap();
//         // created by the macro at the beginning of the test
//         let _other_block_number = runner.provider().block_number().await.unwrap();
//     }
// }
