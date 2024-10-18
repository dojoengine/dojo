use katana_runner::RunnerCtx;
use starknet::macros::short_string;
use starknet::providers::Provider;

#[katana_runner::test(fee = false, accounts = 7)]
fn simple(runner: &RunnerCtx) {
    assert_eq!(runner.accounts().len(), 7);
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(chain_id = short_string!("SN_SEPOLIA"))]
async fn custom_chain_id(runner: &RunnerCtx) {
    let provider = runner.provider();
    let id = provider.chain_id().await.unwrap();
    assert_eq!(id, short_string!("SN_SEPOLIA"));
}

#[katana_runner::test]
fn with_return(_: &RunnerCtx) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[katana_runner::test]
async fn with_async(ctx: &RunnerCtx) -> Result<(), Box<dyn std::error::Error>> {
    let provider = ctx.provider();
    let id = provider.chain_id().await?;
    assert_eq!(id, short_string!("KATANA"));
    Ok(())
}
