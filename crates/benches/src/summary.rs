use std::{fs::OpenOptions, io::Write};

use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct BenchSummary {
    // All times are in miliseconds
    pub name: String,
    pub sending_time: u64,
    pub responses_span: u64,
    pub longest_confirmation_difference: u64,
    pub stats: Option<BenchStats>,
    pub block_times: Vec<i64>,
    pub block_sizes: Vec<u32>,
    pub steps: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct BenchStats {
    pub estimated_tps: f64,
    pub estimated_sps: f64,
    pub relevant_blocks: Vec<(u32, i64)>,
    pub relevant_time: i64,
}

impl BenchSummary {
    pub fn relevant_blocks(&self) -> Vec<(u32, i64)> {
        let mut joined = self
            .block_sizes
            .iter()
            .zip(self.block_times.iter())
            .map(|(s, t)| (*s, *t))
            .collect::<Vec<_>>();

        while let Some((size, _time)) = joined.last() {
            if *size == 0 {
                joined.pop();
            } else {
                break;
            }
        }

        let mut start = 0;
        for (i, (size, _time)) in joined.iter().enumerate().rev() {
            if *size == 0 {
                start = i + 1;
                break;
            }
        }

        joined.drain(start..).collect()
    }

    pub fn estimated_tps(&self) -> f64 {
        let relevant_blocks = self.relevant_blocks();
        let total_transactions = relevant_blocks.iter().map(|(s, _t)| s).sum::<u32>();
        total_transactions as f64 / self.relevant_time() as f64 * 1000.0
    }

    pub fn relevant_time(&self) -> i64 {
        let relevant_blocks = self.relevant_blocks();
        relevant_blocks.iter().map(|(_s, t)| t).sum::<i64>()
    }

    pub fn estimated_sps(&self) -> f64 {
        self.steps as f64 / self.relevant_time() as f64 * 1000.0
    }

    pub fn compute_stats(&mut self) {
        if self.stats.is_none() {
            self.stats = Some(BenchStats {
                estimated_tps: self.estimated_tps(),
                estimated_sps: self.estimated_sps(),
                relevant_blocks: self.relevant_blocks(),
                relevant_time: self.relevant_time(),
            });
        }
    }

    pub async fn dump(&self) {
        let mut file =
            OpenOptions::new().create(true).append(true).open("bench_results.txt").unwrap();

        let mut data = self.clone();
        data.compute_stats();
        writeln!(file, "{}", serde_json::to_string(&data).unwrap()).unwrap();
    }
}

impl std::fmt::Display for BenchSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "sending time: {}", self.sending_time)?;
        writeln!(f, "responses span: {}", self.responses_span)?;
        writeln!(f, "longest confirmation difference: {}", self.longest_confirmation_difference)?;
        writeln!(f, "block times: {:?}", self.block_times)?;
        writeln!(f, "block sizes: {:?}", self.block_sizes)?;
        writeln!(f, "relevant blocks: {:?}", self.relevant_blocks())?;
        writeln!(f, "estimated tps: {}", self.estimated_tps())?;
        Ok(())
    }
}
