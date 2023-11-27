mod binary;
mod builder;
mod compiled;

pub use binary::*;
pub use builder::*;
pub use compiled::*;

#[tokio::test]
async fn test_run() {
    for _ in 0..10 {
        let (_katana_guard, _) = KatanaRunnerBuilder::new().compiled().await.unwrap();
        println!("Restarting server");
    }
}

#[tokio::test]
async fn test_run_binary() {
    for _ in 0..10 {
        let (_katana_guard, _) = KatanaRunnerBuilder::new().binary().unwrap();

        println!("Restarting server");
    }
}
