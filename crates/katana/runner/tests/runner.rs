use katana_runner::RunnerCtx;
use starknet::providers::Provider;

#[katana_runner::test(fee = false, accounts = 7)]
fn simple(runner: &RunnerCtx) {
    assert_eq!(runner.accounts().len(), 7);
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
