//! What the benchmark produces. Separate from both the measuring and the
//! reporting so that neither needs to know about the other.

use {
    alloy_primitives::{Address, U256},
    serde::Serialize,
    std::time::Duration,
};

/// One point in the swept matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Config {
    pub multicall_batch_size: usize,
    /// The values actually handed to `ethrpc`, after normalisation.
    pub ethrpc_batch_size: usize,
    pub ethrpc_concurrency: usize,
    pub ethrpc_batch_delay_ms: u64,
}

impl Config {
    pub fn new(multicall: usize, batch: usize, concurrency: usize, delay_ms: u64) -> Self {
        // `ethrpc` only skips the batching layer for `(0 | 1, 0)`; every other
        // combination goes through `chunks_timeout`, which rejects a chunk size
        // of 0. Normalise here so the report shows what the node really saw.
        let (batch, concurrency) = if batch <= 1 {
            (1, 0)
        } else {
            (batch, concurrency)
        };
        Self {
            multicall_batch_size: multicall,
            ethrpc_batch_size: batch,
            ethrpc_concurrency: concurrency,
            ethrpc_batch_delay_ms: delay_ms,
        }
    }

    pub fn ethrpc(&self) -> ethrpc::Config {
        ethrpc::Config {
            ethrpc_max_batch_size: self.ethrpc_batch_size,
            ethrpc_max_concurrent_requests: self.ethrpc_concurrency,
            ethrpc_batch_delay: Duration::from_millis(self.ethrpc_batch_delay_ms),
        }
    }
}

/// One timed pass over the whole working set.
#[derive(Debug, Serialize)]
pub struct Pass {
    pub wall_ms: u128,
    /// Logical JSON-RPC calls. `ethrpc`'s instrumentation layer sits above its
    /// batching layer, so this counts calls as the caller made them, before any
    /// coalescing into HTTP requests.
    pub calls: u64,
    /// HTTP round-trips, estimated as `calls / ethrpc_batch_size`. Not
    /// measured: nothing below the batching layer is instrumented, and how
    /// full a batch ends up depends on what was queued when it was flushed.
    pub http_estimate: u64,
    /// Mean duration of one logical call. A batched call spans the whole HTTP
    /// request, so this approaches the round-trip time as batches fill up.
    pub mean_call_ms: f64,
    pub ok: usize,
    pub err: usize,
    pub methods: String,
}

#[derive(Debug, Serialize)]
pub struct Measurement {
    #[serde(flatten)]
    pub config: Config,
    pub passes: Vec<Pass>,
    /// Results that differ from the baseline config's.
    pub parity_mismatches: Option<usize>,
    /// Results that differ between this config's own first and last pass. Both
    /// passes took the same code path, so whatever shows up here is a balance
    /// that moved on chain — the yardstick that says how much of
    /// `parity_mismatches` is noise rather than a real difference between the
    /// batched and unbatched paths.
    pub volatile: Option<usize>,
    /// A few pairs that disagree with the baseline, to look up by hand.
    pub examples: Vec<Mismatch>,
}

impl Measurement {
    /// Min, median and max wall time. The min is the most robust of the three;
    /// the median only means anything from three passes up.
    pub fn wall_ms(&self) -> (u128, u128, u128) {
        let mut times: Vec<_> = self.passes.iter().map(|pass| pass.wall_ms).collect();
        times.sort_unstable();
        match times.as_slice() {
            [] => (0, 0, 0),
            times => (times[0], times[times.len() / 2], times[times.len() - 1]),
        }
    }

    pub fn mean(&self, get: impl Fn(&Pass) -> f64) -> f64 {
        if self.passes.is_empty() {
            return 0.0;
        }
        self.passes.iter().map(get).sum::<f64>() / self.passes.len() as f64
    }
}

#[derive(Debug, Serialize)]
pub struct Mismatch {
    pub owner: Address,
    pub token: Address,
    pub baseline: Option<U256>,
    pub actual: Option<U256>,
}

/// Indices of results that disagree, either in value or in whether they
/// succeeded.
pub fn mismatches(baseline: &[Option<U256>], other: &[Option<U256>]) -> Vec<usize> {
    baseline
        .iter()
        .zip(other)
        .enumerate()
        .filter(|(_, (baseline, other))| baseline != other)
        .map(|(index, _)| index)
        .collect()
}
