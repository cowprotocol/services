//! Replays a working set against a node under a matrix of batching configs.

use {
    crate::{fixture::Fixture, metrics::Snapshot},
    account_balances::{BalanceSimulator, Overrides, fetcher_with_overrides},
    alloy_primitives::{Address, U256, address},
    alloy_provider::Provider,
    alloy_rpc_types::{BlockId, BlockNumberOrTag},
    anyhow::{Context, Result},
    balance_overrides::DummyStateOverrider,
    chain::Chain,
    contracts::{GPv2Settlement, support::Balances as SupportBalances},
    reqwest::Url,
    serde::Serialize,
    std::{
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    },
};

/// `GPv2Settlement` is deployed at the same address on every supported network.
const SETTLEMENT: Address = address!("0x9008D19f58AAbD9eD0D60971565AA8510560ab41");

#[derive(Debug, clap::Args)]
pub struct Args {
    #[clap(long, env = "NODE_URL")]
    node_url: Url,

    #[clap(long, short)]
    fixture: PathBuf,

    /// Queries per `Multicall3` call, swept in order. `0` reads every balance
    /// individually and is the baseline the other configs are compared to.
    #[clap(
        long,
        value_delimiter = ',',
        default_value = "0,10,25,50,100,200",
        allow_negative_numbers = false
    )]
    multicall_batch_size: Vec<usize>,

    /// JSON-RPC requests coalesced into one HTTP request by the `ethrpc` batch
    /// layer. `0` or `1` removes the layer entirely (and forces concurrency 0).
    #[clap(long, value_delimiter = ',', default_value = "20")]
    ethrpc_batch_size: Vec<usize>,

    /// Concurrent in-flight batches. `0` is unlimited.
    #[clap(long, value_delimiter = ',', default_value = "10")]
    ethrpc_concurrency: Vec<usize>,

    /// Nagle delay the batch layer waits for a batch to fill up.
    #[clap(long, value_delimiter = ',', default_value = "0")]
    ethrpc_batch_delay_ms: Vec<u64>,

    /// Timed passes per config.
    #[clap(long, default_value = "3")]
    repeat: usize,

    /// Untimed passes over the whole working set before the matrix starts, so
    /// that no config pays for a cold node state cache that the others don't.
    #[clap(long, default_value = "1")]
    warmup: usize,

    /// Only use the first N pairs of the fixture.
    #[clap(long)]
    queries: Option<usize>,

    /// Block every read is pinned to: `finalized`, `latest`, a number, or
    /// `none` to follow the chain the way production does. Tags are resolved to
    /// a concrete number once, so the whole matrix sees identical state and
    /// rebasing tokens stop showing up as disagreements.
    #[clap(long, default_value = "finalized")]
    block: BlockSpec,

    #[clap(long, default_value_t = SETTLEMENT)]
    settlement: Address,

    /// Skip comparing every config's results against the baseline's.
    #[clap(long)]
    skip_parity: bool,

    #[clap(long)]
    json: Option<PathBuf>,

    #[clap(long)]
    csv: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
enum BlockSpec {
    /// Follow the chain, as production does.
    None,
    Tag(BlockNumberOrTag),
    Number(u64),
}

impl std::str::FromStr for BlockSpec {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" | "latest-unpinned" => Ok(Self::None),
            "finalized" => Ok(Self::Tag(BlockNumberOrTag::Finalized)),
            "safe" => Ok(Self::Tag(BlockNumberOrTag::Safe)),
            "latest" => Ok(Self::Tag(BlockNumberOrTag::Latest)),
            number => number.parse().map(Self::Number).map_err(|_| {
                format!("expected finalized, safe, latest, none or a block number, got {number}")
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct Config {
    multicall_batch_size: usize,
    /// The values actually handed to `ethrpc`, after normalisation.
    ethrpc_batch_size: usize,
    ethrpc_concurrency: usize,
    ethrpc_batch_delay_ms: u64,
}

impl Config {
    fn new(multicall: usize, batch: usize, concurrency: usize, delay_ms: u64) -> Self {
        // `ethrpc` only skips the batching layer for `(0 | 1, 0)`; every other
        // combination goes through `chunks_timeout`, which rejects a chunk size
        // of 0. Normalise here so the table reports what the node really saw.
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

    fn ethrpc(&self) -> ethrpc::Config {
        ethrpc::Config {
            ethrpc_max_batch_size: self.ethrpc_batch_size,
            ethrpc_max_concurrent_requests: self.ethrpc_concurrency,
            ethrpc_batch_delay: Duration::from_millis(self.ethrpc_batch_delay_ms),
        }
    }
}

#[derive(Debug, Serialize)]
struct Run {
    wall_ms: u128,
    /// Logical JSON-RPC calls. `ethrpc`'s instrumentation layer sits above its
    /// batching layer, so this counts calls as the caller made them, before any
    /// coalescing into HTTP requests.
    calls: u64,
    /// HTTP round-trips, estimated as `calls / ethrpc_batch_size`. Not
    /// measured: nothing below the batching layer is instrumented, and how
    /// full a batch ends up depends on what was queued when it was flushed.
    http_estimate: u64,
    /// Mean duration of one logical call. A batched call spans the whole HTTP
    /// request, so this approaches the round-trip time as batches fill up.
    mean_call_ms: f64,
    ok: usize,
    err: usize,
    methods: String,
}

#[derive(Debug, Serialize)]
struct Measurement {
    #[serde(flatten)]
    config: Config,
    runs: Vec<Run>,
    /// Results that differ from the baseline config's.
    parity_mismatches: Option<usize>,
    /// Results that differ between this config's own first and last pass. Both
    /// passes took the same code path, so whatever shows up here is a balance
    /// that moved on chain — the yardstick that says how much of
    /// `parity_mismatches` is noise rather than a real difference between the
    /// batched and unbatched paths.
    volatile: Option<usize>,
    /// A few pairs that disagree with the baseline, to look up by hand.
    examples: Vec<Mismatch>,
}

#[derive(Debug, Serialize)]
struct Mismatch {
    owner: Address,
    token: Address,
    baseline: Option<U256>,
    actual: Option<U256>,
}

impl Measurement {
    fn wall_ms(&self) -> (u128, u128, u128) {
        let mut times: Vec<_> = self.runs.iter().map(|run| run.wall_ms).collect();
        times.sort_unstable();
        match times.as_slice() {
            [] => (0, 0, 0),
            times => (times[0], times[times.len() / 2], times[times.len() - 1]),
        }
    }

    fn mean(&self, get: impl Fn(&Run) -> f64) -> f64 {
        if self.runs.is_empty() {
            return 0.0;
        }
        self.runs.iter().map(get).sum::<f64>() / self.runs.len() as f64
    }
}

pub async fn run(args: Args) -> Result<()> {
    let mut fixture = Fixture::load(&args.fixture)?;
    if let Some(limit) = args.queries {
        fixture.pairs.truncate(limit);
    }
    let queries = fixture.queries();
    anyhow::ensure!(!queries.is_empty(), "empty working set");

    let (tokens, owners) = fixture.diversity();
    println!(
        "working set: {} pairs, {tokens} distinct tokens, {owners} distinct owners (fixture \
         network {}, dumped {})",
        queries.len(),
        fixture.network,
        fixture.dumped_at,
    );

    // One provider outside the matrix for the setup calls, so they don't land in
    // any config's request counts.
    let control = ethrpc::web3(Default::default(), &args.node_url, Some("setup"));
    let chain_id = control
        .provider
        .get_chain_id()
        .await
        .context("could not read the chain ID")?;
    match Chain::try_from(chain_id) {
        Ok(chain) if chain.as_str() != fixture.network => println!(
            "WARNING: node is {} but the fixture was dumped from {} — the pairs are meaningless \
             on this chain",
            chain.as_str(),
            fixture.network,
        ),
        Ok(_) => (),
        Err(_) => println!("WARNING: unknown chain ID {chain_id}"),
    }

    // Resolve the tag to a number once. Pinning to a tag would let the block
    // advance underneath the matrix, which is the thing we are trying to stop.
    let block = match args.block {
        BlockSpec::None => None,
        BlockSpec::Number(number) => Some(number),
        BlockSpec::Tag(tag) => Some(
            control
                .provider
                .get_block_by_number(tag)
                .await
                .with_context(|| format!("could not fetch the {tag} block"))?
                .with_context(|| format!("node has no {tag} block"))?
                .header
                .number,
        ),
    };
    match block {
        Some(number) => {
            let latest = control.provider.get_block_number().await?;
            println!(
                "pinned to block {number} ({} behind latest {latest})",
                latest.saturating_sub(number),
            );
        }
        None => println!("not pinned: reads follow the chain, as production does"),
    }

    let settlement = GPv2Settlement::GPv2Settlement::new(args.settlement, control.provider.clone());
    let vault_relayer = settlement
        .vaultRelayer()
        .call()
        .await
        .context("could not read the vault relayer; is --settlement right for this chain?")?;
    println!(
        "settlement {} vault relayer {vault_relayer}",
        args.settlement
    );

    let simulator = || {
        BalanceSimulator::new(
            settlement.clone(),
            // Only the interaction-free path is benchmarked, which never
            // touches the support contract.
            SupportBalances::Instance::new(Address::ZERO, control.provider.clone()),
            vault_relayer,
            Arc::new(DummyStateOverrider),
        )
    };

    let overrides = |multicall_batch_size| Overrides {
        multicall_batch_size: Some(multicall_batch_size),
        block: block.map(BlockId::number),
    };

    if args.warmup > 0 {
        let fetcher = fetcher_with_overrides(&control, simulator(), overrides(50));
        for pass in 1..=args.warmup {
            let start = Instant::now();
            let results = fetcher.get_balances(&queries).await;
            let ok = results.iter().filter(|result| result.is_ok()).count();
            println!(
                "warmup {pass}/{}: {:?}, {ok}/{} ok",
                args.warmup,
                start.elapsed(),
                results.len(),
            );
        }
    }

    let mut configs: Vec<Config> = Vec::new();
    for multicall in &args.multicall_batch_size {
        for batch in &args.ethrpc_batch_size {
            for concurrency in &args.ethrpc_concurrency {
                for delay in &args.ethrpc_batch_delay_ms {
                    let config = Config::new(*multicall, *batch, *concurrency, *delay);
                    // Normalisation collapses several requested combinations
                    // onto the same one; benchmarking it twice tells us nothing.
                    if !configs.contains(&config) {
                        configs.push(config);
                    }
                }
            }
        }
    }

    let mut baseline: Option<Vec<Option<U256>>> = None;
    let mut measurements = Vec::new();

    for config in configs {
        // A fresh provider per config, otherwise the batching parameters of the
        // previous one keep applying.
        let web3 = ethrpc::web3(config.ethrpc(), &args.node_url, Some("bench"));
        let fetcher =
            fetcher_with_overrides(&web3, simulator(), overrides(config.multicall_batch_size));

        // Opens the HTTP connections and resolves the one-off `Multicall3` code
        // lookup, neither of which belongs in a timed pass.
        fetcher.get_balances(&queries[..queries.len().min(2)]).await;

        let mut runs = Vec::new();
        let mut first: Option<Vec<Option<U256>>> = None;
        let mut last: Option<Vec<Option<U256>>> = None;
        for _ in 0..args.repeat {
            let before = Snapshot::take();
            let start = Instant::now();
            let results = fetcher.get_balances(&queries).await;
            let wall_ms = start.elapsed().as_millis();
            let delta = before.delta(&Snapshot::take());

            let ok = results.iter().filter(|result| result.is_ok()).count();
            let calls = delta.total_requests();
            runs.push(Run {
                wall_ms,
                calls,
                http_estimate: calls.div_ceil(config.ethrpc_batch_size.max(1) as u64),
                mean_call_ms: delta.mean_request_seconds() * 1000.0,
                ok,
                err: results.len() - ok,
                methods: delta.methods(),
            });
            let values: Vec<_> = results
                .iter()
                .map(|result| result.as_ref().ok().copied())
                .collect();
            if first.is_none() {
                first = Some(values.clone());
            }
            last = Some(values);
        }

        let (parity_mismatches, examples) = match (args.skip_parity, &baseline, &last) {
            (false, Some(baseline), Some(last)) => (
                Some(mismatches(baseline, last).len()),
                mismatches(baseline, last)
                    .into_iter()
                    .take(5)
                    .map(|index| Mismatch {
                        owner: queries[index].owner,
                        token: queries[index].token,
                        baseline: baseline[index],
                        actual: last[index],
                    })
                    .collect(),
            ),
            _ => (None, Vec::new()),
        };
        let volatile = first
            .as_ref()
            .zip(last.as_ref())
            .filter(|_| args.repeat > 1)
            .map(|(first, last)| mismatches(first, last).len());
        if baseline.is_none() {
            baseline = last;
        }

        let measurement = Measurement {
            config,
            runs,
            parity_mismatches,
            volatile,
            examples,
        };
        let (min, med, max) = measurement.wall_ms();
        println!(
            "mc={} rpc_batch={} conc={} delay={}ms  wall {min}/{med}/{max} ms  calls={:.0}  \
             call={:.1}ms",
            config.multicall_batch_size,
            config.ethrpc_batch_size,
            config.ethrpc_concurrency,
            config.ethrpc_batch_delay_ms,
            measurement.mean(|run| run.calls as f64),
            measurement.mean(|run| run.mean_call_ms),
        );
        measurements.push(measurement);
    }

    report(&measurements);

    if let Some(path) = &args.json {
        std::fs::write(path, serde_json::to_vec_pretty(&measurements)?)?;
        println!("wrote {}", path.display());
    }
    if let Some(path) = &args.csv {
        std::fs::write(path, csv(&measurements))?;
        println!("wrote {}", path.display());
    }

    Ok(())
}

/// Indices of results that disagree, either in value or in whether they
/// succeeded.
fn mismatches(baseline: &[Option<U256>], other: &[Option<U256>]) -> Vec<usize> {
    baseline
        .iter()
        .zip(other)
        .enumerate()
        .filter(|(_, (baseline, other))| baseline != other)
        .map(|(index, _)| index)
        .collect()
}

const HEADERS: [&str; 13] = [
    "mc",
    "rpc_batch",
    "conc",
    "delay",
    "min_ms",
    "med_ms",
    "max_ms",
    "calls",
    "http~",
    "call_ms",
    "err",
    "mism",
    "moved",
];
const WIDTHS: [usize; 13] = [4, 9, 4, 5, 7, 7, 7, 6, 6, 7, 5, 5, 5];

fn report(measurements: &[Measurement]) {
    println!();
    println!("{}", row(&HEADERS.map(str::to_owned)));
    println!("{}", row(&WIDTHS.map(|width| "─".repeat(width).to_owned())));
    for measurement in measurements {
        let (min, med, max) = measurement.wall_ms();
        println!(
            "{}",
            row(&[
                measurement.config.multicall_batch_size.to_string(),
                measurement.config.ethrpc_batch_size.to_string(),
                measurement.config.ethrpc_concurrency.to_string(),
                measurement.config.ethrpc_batch_delay_ms.to_string(),
                min.to_string(),
                med.to_string(),
                max.to_string(),
                format!("{:.0}", measurement.mean(|run| run.calls as f64)),
                format!("{:.0}", measurement.mean(|run| run.http_estimate as f64)),
                format!("{:.1}", measurement.mean(|run| run.mean_call_ms)),
                format!("{:.0}", measurement.mean(|run| run.err as f64)),
                optional(measurement.parity_mismatches),
                optional(measurement.volatile),
            ])
        );
    }
    println!(
        "\nmc      queries per Multicall3 call; 0 reads them individually and is the parity \
         baseline\ncalls   logical JSON-RPC calls per pass, before the ethrpc batching layer \
         coalesces them\nhttp~   HTTP round-trips, estimated as calls/rpc_batch — not \
         measured\ncall_ms mean duration of one logical call; for batched calls this spans the \
         whole HTTP request\nmism    results differing from the baseline config\nmoved   results \
         differing between this config's own first and last pass — same code path, so this is \
         on-chain movement and the noise floor for mism"
    );

    for measurement in measurements {
        if measurement.examples.is_empty() {
            continue;
        }
        println!(
            "\nmc={} disagrees with the baseline on:",
            measurement.config.multicall_batch_size
        );
        for example in &measurement.examples {
            let show =
                |value: Option<U256>| value.map_or("failed".to_owned(), |value| value.to_string());
            println!(
                "  owner {} token {}  baseline {}  actual {}",
                example.owner,
                example.token,
                show(example.baseline),
                show(example.actual),
            );
        }
    }
}

fn optional(value: Option<usize>) -> String {
    value.map_or("-".to_owned(), |value| value.to_string())
}

fn row(cells: &[String; 13]) -> String {
    let mut out = String::new();
    for (cell, width) in cells.iter().zip(WIDTHS) {
        out.push_str(&format!("{cell:>width$}  "));
    }
    out.trim_end().to_owned()
}

fn csv(measurements: &[Measurement]) -> String {
    let mut out = String::from(
        "multicall_batch_size,ethrpc_batch_size,ethrpc_concurrency,ethrpc_batch_delay_ms,run,\
         wall_ms,calls,http_estimate,mean_call_ms,ok,err,parity_mismatches\n",
    );
    for measurement in measurements {
        for (index, run) in measurement.runs.iter().enumerate() {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{:.3},{},{},{}\n",
                measurement.config.multicall_batch_size,
                measurement.config.ethrpc_batch_size,
                measurement.config.ethrpc_concurrency,
                measurement.config.ethrpc_batch_delay_ms,
                index,
                run.wall_ms,
                run.calls,
                run.http_estimate,
                run.mean_call_ms,
                run.ok,
                run.err,
                measurement
                    .parity_mismatches
                    .map_or(String::new(), |count| count.to_string()),
            ));
        }
    }
    out
}
