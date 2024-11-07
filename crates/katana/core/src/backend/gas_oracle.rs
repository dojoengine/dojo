use std::collections::VecDeque;
use std::fmt::Debug;
use std::future::IntoFuture;
use std::pin::Pin;

use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_eth::BlockNumberOrTag;
use alloy_transport::Transport;
use anyhow::{Context, Ok};
use futures::Future;
use katana_primitives::block::GasPrices;
use tokio::time::Duration;
use url::Url;

const BUFFER_SIZE: usize = 60;
const INTERVAL: Duration = Duration::from_secs(60);
const ONE_GWEI: u128 = 1_000_000_000;

// TODO: implement a proper gas oracle function - sample the l1 gas and data gas prices
// currently this just return the hardcoded value set from the cli or if not set, the default value.
#[derive(Debug)]
pub enum L1GasOracle {
    Fixed(FixedL1GasOracle),
    Sampled(SampledL1GasOracle),
}

#[derive(Debug)]
pub struct FixedL1GasOracle {
    gas_prices: GasPrices,
    data_gas_prices: GasPrices,
}

#[derive(Debug, Default, Clone)]
pub struct SampledL1GasOracle {
    gas_prices: GasPrices,
    data_gas_prices: GasPrices,
}

#[derive(Debug, Clone)]
pub struct GasOracleWorker {
    pub l1_oracle: SampledL1GasOracle,
    pub l1_provider_url: Option<Url>,
}

impl L1GasOracle {
    pub fn fixed(gas_prices: GasPrices, data_gas_prices: GasPrices) -> Self {
        L1GasOracle::Fixed(FixedL1GasOracle { gas_prices, data_gas_prices })
    }

    pub fn sampled() -> Self {
        L1GasOracle::Sampled(SampledL1GasOracle {
            gas_prices: GasPrices::default(),
            data_gas_prices: GasPrices::default(),
        })
    }

    /// Returns the current gas prices.
    pub fn current_gas_prices(&self) -> GasPrices {
        match self {
            L1GasOracle::Fixed(fixed) => fixed.gas_prices.clone(),
            L1GasOracle::Sampled(sampled) => sampled.gas_prices.clone(),
        }
    }

    /// Returns the current data gas prices.
    pub fn current_data_gas_prices(&self) -> GasPrices {
        match self {
            L1GasOracle::Fixed(fixed) => fixed.data_gas_prices.clone(),
            L1GasOracle::Sampled(sampled) => sampled.data_gas_prices.clone(),
        }
    }
}

impl SampledL1GasOracle {
    pub fn current_data_gas_prices(&self) -> GasPrices {
        self.data_gas_prices.clone()
    }

    pub fn current_gas_prices(&self) -> GasPrices {
        self.gas_prices.clone()
    }
}

impl FixedL1GasOracle {
    pub fn current_data_gas_prices(&self) -> GasPrices {
        self.data_gas_prices.clone()
    }

    pub fn current_gas_prices(&self) -> GasPrices {
        self.gas_prices.clone()
    }
}

async fn update_gas_price<P: Provider<T>, T: Transport + Clone>(
    l1_oracle: &mut SampledL1GasOracle,
    provider: P,
    buffer: &mut GasPriceBuffer,
) -> anyhow::Result<()> {
    // Attempt to get the gas price from L1
    let last_block_number = provider.get_block_number().await?;
    let fee_history =
        provider.get_fee_history(1, BlockNumberOrTag::Number(last_block_number), &[]).await?;

    let latest_gas_price = fee_history.base_fee_per_gas.last().context("Getting eth gas price")?;
    buffer.add_sample(*latest_gas_price);

    let blob_fee_history = fee_history.base_fee_per_blob_gas;
    let avg_blob_base_fee = blob_fee_history.iter().last().context("Getting blob gas price")?;

    let avg_blob_fee_eth = *avg_blob_base_fee;
    let avg_blob_fee_strk = *avg_blob_base_fee + ONE_GWEI;

    let avg_gas_price = GasPrices {
        eth: buffer.average(),
        // The price of gas on Starknet is set to the average of the last 60 gas price samples, plus
        // 1 gwei.
        strk: buffer.average() + ONE_GWEI,
    };
    let avg_blob_price = GasPrices { eth: avg_blob_fee_eth, strk: avg_blob_fee_strk };

    l1_oracle.gas_prices = avg_gas_price;
    l1_oracle.data_gas_prices = avg_blob_price;
    Ok(())
}

impl GasOracleWorker {
    pub fn new(l1_provider_url: Option<Url>) -> Self {
        Self { l1_oracle: SampledL1GasOracle::default(), l1_provider_url }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut buffer = GasPriceBuffer::new();
        let provider =
            ProviderBuilder::new().on_http(self.l1_provider_url.clone().expect("No provided URL"));
        // every 60 seconds, Starknet samples the base price of gas and data gas on L1
        let mut interval = tokio::time::interval(INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // tick every 60 seconds
                _ = interval.tick() => {
                    if let Err(e) = update_gas_price(&mut self.l1_oracle, provider.clone(), &mut buffer).await {
                        eprintln!("Error running the gas oracle: {:?}", e);
                    }
                    // (provisionary)
                    println!("{:?}", self.l1_oracle.current_gas_prices());
                }
            }
        }
    }
}

impl IntoFuture for GasOracleWorker {
    type Output = anyhow::Result<()>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(mut self) -> Self::IntoFuture {
        Box::pin(async move { self.run().await })
    }
}

// Buffer to store the last 60 gas price samples
#[derive(Debug)]
pub struct GasPriceBuffer {
    buffer: VecDeque<u128>,
}

impl GasPriceBuffer {
    fn new() -> Self {
        Self { buffer: VecDeque::with_capacity(BUFFER_SIZE) }
    }

    fn add_sample(&mut self, sample: u128) {
        if self.buffer.len() == BUFFER_SIZE {
            // remove oldest sample if buffer is full
            self.buffer.pop_front();
        }
        self.buffer.push_back(sample);
    }

    fn average(&self) -> u128 {
        if self.buffer.is_empty() {
            return 0;
        }
        let sum: u128 = self.buffer.iter().sum();
        sum / self.buffer.len() as u128
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    // Test the buffer functionality separately
    #[test]
    fn test_buffer_size_limit() {
        let mut buffer = GasPriceBuffer::new();

        // Add more samples than the buffer size
        for i in 0..BUFFER_SIZE + 10 {
            buffer.add_sample(i as u128);
        }

        // Check if buffer size is maintained
        assert_eq!(buffer.buffer.len(), BUFFER_SIZE);

        // Check if oldest values were removed (should start from 10)
        assert_eq!(*buffer.buffer.front().unwrap(), 10);
    }

    #[test]
    fn test_empty_buffer_average() {
        let buffer = GasPriceBuffer::new();
        assert_eq!(buffer.average(), 0);
    }

    #[test]
    fn test_buffer_single_sample_average() {
        let mut buffer = GasPriceBuffer::new();
        buffer.add_sample(100);
        assert_eq!(buffer.average(), 100);
    }

    #[test]
    fn test_bufffer_multiple_samples_average() {
        let mut buffer = GasPriceBuffer::new();
        // Add some test values
        let test_values = vec![100, 200, 300];
        for value in test_values.iter() {
            buffer.add_sample(*value);
        }

        let expected_avg = 200; // (100 + 200 + 300) / 3
        assert_eq!(buffer.average(), expected_avg);
    }

    #[tokio::test]
    async fn test_worker_interval() {
        use tokio::time::Duration;
        const TEST_DURATION: Duration = Duration::from_secs(30); // To test if it actually updates every 60 secs, like in the starknet doc.(https://docs.starknet.io/architecture-and-concepts/network-architecture/fee-mechanism/#calculation_of_gas_costs)

        let url = Url::parse("https://eth.merkle.io/").expect("error url");
        let mut worker = GasOracleWorker::new(Some(url));

        let worker_task = tokio::spawn(async move {
            // this fails because of an alloy error:
            // Quote: <"error sending request for url (https://eth.merkle.io/)
            // Caused by:
            //   0: error sending request for url (https://eth.merkle.io/)
            //   1: client error (Connect)
            //   2: invalid URL, scheme is not http">
            if let Err(e) = worker.run().await {
                eprintln!("Worker failed: {}", e);
            }
        });

        // Allow the test to run for a while to observe intervals
        tokio::time::sleep(TEST_DURATION).await;

        // Clean up
        worker_task.abort();
    }
}
