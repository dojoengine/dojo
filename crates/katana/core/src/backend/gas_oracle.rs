use std::backtrace::Backtrace;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::Arc;

use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_eth::BlockNumberOrTag;
use alloy_transport::Transport;
use anyhow::{Context, Ok};
use katana_primitives::block::GasPrices;
use katana_tasks::TaskSpawner;
use tokio::sync::Mutex;
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
    prices: Arc<Mutex<SampledPrices>>,
    l1_provider: Option<Url>,
}

#[derive(Debug, Default)]
pub struct SampledPrices {
    gas_prices: GasPrices,
    data_gas_prices: GasPrices,
}

#[derive(Debug, Clone)]
pub struct GasOracleWorker {
    pub prices: Arc<Mutex<SampledPrices>>,
    pub l1_provider_url: Url,
    pub price_buffer: GasPriceBuffer,
}

impl L1GasOracle {
    pub fn fixed(gas_prices: GasPrices, data_gas_prices: GasPrices) -> Self {
        L1GasOracle::Fixed(FixedL1GasOracle { gas_prices, data_gas_prices })
    }

    pub fn sampled(l1_provider: Option<Url>) -> Self {
        let prices: Arc<Mutex<SampledPrices>> = Arc::new(Mutex::new(SampledPrices::default()));
        L1GasOracle::Sampled(SampledL1GasOracle { prices, l1_provider })
    }

    /// Returns the current gas prices.
    pub fn current_gas_prices(&self) -> GasPrices {
        match self {
            L1GasOracle::Fixed(fixed) => fixed.current_gas_prices(),
            L1GasOracle::Sampled(sampled) => sampled
                .prices
                .try_lock()
                .map(|prices| prices.gas_prices.clone())
                .unwrap_or_else(|_| GasPrices::default()),
        }
    }

    /// Returns the current data gas prices.
    pub fn current_data_gas_prices(&self) -> GasPrices {
        match self {
            L1GasOracle::Fixed(fixed) => fixed.current_data_gas_prices(),
            L1GasOracle::Sampled(sampled) => sampled
                .prices
                .try_lock()
                .map(|prices| prices.data_gas_prices.clone())
                .unwrap_or_else(|_| GasPrices::default()),
        }
    }

    pub fn run_worker(&self, task_spawner: TaskSpawner) {
        match self {
            Self::Fixed(..) => {}
            Self::Sampled(oracle) => {
                let mut worker =
                    GasOracleWorker::new(oracle.prices.clone(), oracle.l1_provider.clone());
                task_spawner
                    .build_task()
                    .graceful_shutdown()
                    .name("L1 Gas Oracle worker")
                    .spawn(async move { worker.run().await });
            }
        }
    }
}

impl SampledL1GasOracle {
    pub async fn current_data_gas_prices(&self) -> GasPrices {
        self.prices.lock().await.data_gas_prices.clone()
    }

    pub async fn current_gas_prices(&self) -> GasPrices {
        self.prices.lock().await.gas_prices.clone()
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
    l1_oracle: &mut SampledPrices,
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

    let avg_gas_price = GasPrices { eth: buffer.average(), strk: buffer.average() + ONE_GWEI };
    let avg_blob_price = GasPrices { eth: avg_blob_fee_eth, strk: avg_blob_fee_strk };

    l1_oracle.gas_prices = avg_gas_price;
    l1_oracle.data_gas_prices = avg_blob_price;
    Ok(())
}

impl GasOracleWorker {
    pub fn new(prices: Arc<Mutex<SampledPrices>>, l1_provider_url: Option<Url>) -> Self {
        Self {
            prices,
            l1_provider_url: l1_provider_url.unwrap(),
            price_buffer: GasPriceBuffer::new(),
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut buffer = GasPriceBuffer::new();
        let provider = ProviderBuilder::new().on_http(self.l1_provider_url.clone());
        // every 60 seconds, Starknet samples the base price of gas and data gas on L1
        let mut interval = tokio::time::interval(INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            // Wait for the interval to tick
            interval.tick().await;

            // Attempt to update the gas price
            let mut prices = self.prices.lock().await;
            if let Err(e) = update_gas_price(&mut prices, provider.clone(), &mut buffer).await {
                let trace = Backtrace::capture();
                eprintln!("Error running the gas oracle: {:?}, Backtrace:\n{:?}", e, trace);
            }
        }
    }

    pub async fn update_once(&mut self) -> anyhow::Result<()> {
        let provider = ProviderBuilder::new().on_http(self.l1_provider_url.clone());

        let mut prices = self.prices.lock().await;

        update_gas_price(&mut prices, provider.clone(), &mut self.price_buffer).await
    }
}

// Buffer to store the last 60 gas price samples
#[derive(Debug, Clone)]
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
        let test_values = [100, 200, 300];
        for value in test_values.iter() {
            buffer.add_sample(*value);
        }

        let expected_avg = 200; // (100 + 200 + 300) / 3
        assert_eq!(buffer.average(), expected_avg);
    }

    #[tokio::test]
    async fn test_gas_oracle() {
        let url = Url::parse("https://eth.merkle.io/").expect("Invalid URL");
        let oracle = L1GasOracle::sampled(Some(url.clone()));

        let shared_prices = match &oracle {
            L1GasOracle::Sampled(sampled) => sampled.prices.clone(),
            _ => panic!("Expected sampled oracle"),
        };

        let mut worker = GasOracleWorker::new(shared_prices.clone(), Some(url));

        for i in 0..3 {
            let initial_gas_prices = oracle.current_gas_prices();

            // Verify initial state for first iteration
            if i == 0 {
                assert_eq!(
                    initial_gas_prices,
                    GasPrices { eth: 0, strk: 0 },
                    "First iteration should start with zero prices"
                );
            }

            worker.update_once().await.expect("Failed to update prices");

            let updated_gas_prices = oracle.current_gas_prices();
            let updated_data_gas_prices = oracle.current_data_gas_prices();

            // Verify gas prices
            assert!(updated_gas_prices.eth > 0, "ETH gas price should be non-zero");
            assert_eq!(
                updated_gas_prices.strk,
                updated_gas_prices.eth + ONE_GWEI,
                "STRK price should be ETH price + 1 GWEI"
            );

            assert!(updated_data_gas_prices.eth > 0, "ETH data gas price should be non-zero");
            assert_eq!(
                updated_data_gas_prices.strk,
                updated_data_gas_prices.eth + ONE_GWEI,
                "STRK data gas price should be ETH price + 1 GWEI"
            );

            // For iterations after the first, verify that prices have been updated
            if i > 0 {
                // Give some flexibility for price changes
                if initial_gas_prices.eth != 0 {
                    assert!(
                        initial_gas_prices.eth != updated_gas_prices.eth
                            || initial_gas_prices.strk != updated_gas_prices.strk,
                        "Prices should potentially change between updates"
                    );
                }
            }

            // ETH current avg blocktime is ~12 secs so we need a delay to wait for block creation
            tokio::time::sleep(std::time::Duration::from_secs(9)).await;
        }
    }
}
