mod binary;
mod builder;
mod compiled;

pub use binary::*;
pub use builder::*;
pub use compiled::*;

#[cfg(test)]
mod tests {
    use super::*;
    use starknet::providers::Provider;

    #[tokio::test]
    async fn test_run() {
        let (_katana_guard, long_lived_provider) =
            KatanaRunnerBuilder::new().with_port(21370).binary().unwrap();

        for _ in 0..10 {
            let (_katana_guard, provider) = KatanaRunnerBuilder::new().compiled().await.unwrap();

            let _block_number = provider.block_number().await.unwrap();
            let _other_block_number = long_lived_provider.block_number().await.unwrap();

            println!("Restarting server");
        }
    }

    #[tokio::test]
    async fn test_run_binary() {
        let (_katana_guard, long_lived_provider) =
            KatanaRunnerBuilder::new().with_port(22137).binary().unwrap();

        for _ in 0..10 {
            let (_katana_guard, provider) = KatanaRunnerBuilder::new().binary().unwrap();

            let _block_number = provider.block_number().await.unwrap();
            let _other_block_number = long_lived_provider.block_number().await.unwrap();

            println!("Restarting server");
        }
    }
}
