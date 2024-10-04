#[macro_export]
macro_rules! retry {
    // The macro takes an async block as an input
    ($func:expr) => {{
        // Set the maximum number of retries
        const MAX_RETRIES: usize = 20;

        // Set the delay between retries in milliseconds (adjust as needed)
        const RETRY_DELAY_MS: u64 = 1000;

        let mut retry_count = 0;

        loop {
            match $func.await {
                Ok(result) => break Ok(result), // If the function succeeds, break the loop and return the result
                Err(err) => {
                    tracing::warn!("Error: {}", err);

                    // Check if the maximum number of retries has been reached
                    if retry_count >= MAX_RETRIES {
                        break Err(err);
                    }

                    // Increment the retry count
                    retry_count += 1;
                    tracing::info!("Retrying... ({}/{})", retry_count, MAX_RETRIES);
                    // Wait before retrying
                    tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                }
            }
        }
    }};
}
