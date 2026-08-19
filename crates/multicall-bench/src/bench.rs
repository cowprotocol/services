//! Replays a working set against a node under a matrix of batching configs.
//!
//! Nothing in here formats output — see `report`. The one function that is
//! actually timed is [`timed_pass`].

use {
    crate::{
        fixture::Fixture,
        metrics::Snapshot,
        report,
        results::{Config, Measurement, Mismatch, Pass, mismatches},
    },
    account_balances::{
        BalanceFetching,
        BalanceSimulator,
        Overrides,
        Query,
        fetcher_with_overrides,
    },
    alloy_primitives::{Address, U256, address},
    alloy_provider::Provider,
    alloy_rpc_types::{BlockId, BlockNumberOrTag},
    anyhow::{Context, Result, ensure},
    balance_overrides::DummyStateOverrider,
    chain::Chain,
    contracts::{GPv2Settlement, support::Balances as SupportBalances},
    ethrpc::Web3,
    reqwest::Url,
    std::{path::PathBuf, sync::Arc, time::Instant},
};

/// `GPv2Settlement` is deployed at the same address on every supported network.
const SETTLEMENT: Address = address!("0x9008D19f58AAbD9eD0D60971565AA8510560ab41");

/// Batch size the warmup passes use. Warmup only has to touch every pair so the
/// node caches its state; the cheapest shape does that fastest.
const WARMUP_BATCH_SIZE: usize = 50;

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
pub enum BlockSpec {
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

/// Everything resolved from the node before any measuring starts. The setup
/// calls go through their own provider so they cannot land in a config's
/// request counts.
pub struct Setup {
    control: Web3,
    settlement: GPv2Settlement::Instance,
    vault_relayer: Address,
    /// Resolved block number the reads are pinned to, `None` to follow the
    /// chain.
    pub block: Option<u64>,
    pub latest_block: u64,
    pub chain_id: u64,
}

impl Setup {
    async fn resolve(args: &Args) -> Result<Self> {
        let control = ethrpc::web3(Default::default(), &args.node_url, Some("setup"));
        let chain_id = control
            .provider
            .get_chain_id()
            .await
            .context("could not read the chain ID")?;
        let latest_block = control.provider.get_block_number().await?;

        // Resolve a tag to a number once. Pinning to the tag itself would let
        // the block advance underneath the matrix, which is what we are trying
        // to stop.
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

        let settlement =
            GPv2Settlement::GPv2Settlement::new(args.settlement, control.provider.clone());
        let vault_relayer =
            settlement.vaultRelayer().call().await.context(
                "could not read the vault relayer; is --settlement right for this chain?",
            )?;

        Ok(Self {
            control,
            settlement,
            vault_relayer,
            block,
            latest_block,
            chain_id,
        })
    }

    pub fn chain(&self) -> Option<Chain> {
        Chain::try_from(self.chain_id).ok()
    }

    pub fn vault_relayer(&self) -> Address {
        self.vault_relayer
    }

    fn simulator(&self) -> BalanceSimulator {
        BalanceSimulator::new(
            self.settlement.clone(),
            // Only the interaction-free path is benchmarked, which never
            // touches the support contract.
            SupportBalances::Instance::new(Address::ZERO, self.control.provider.clone()),
            self.vault_relayer,
            Arc::new(DummyStateOverrider),
        )
    }

    fn fetcher(&self, web3: &Web3, multicall_batch_size: usize) -> Arc<dyn BalanceFetching> {
        fetcher_with_overrides(
            web3,
            self.simulator(),
            Overrides {
                multicall_batch_size: Some(multicall_batch_size),
                block: self.block.map(BlockId::number),
            },
        )
    }
}

pub async fn run(args: Args) -> Result<()> {
    let mut fixture = Fixture::load(&args.fixture)?;
    if let Some(limit) = args.queries {
        fixture.pairs.truncate(limit);
    }
    let queries = fixture.queries();
    ensure!(!queries.is_empty(), "empty working set");
    report::working_set(&fixture, queries.len());

    let setup = Setup::resolve(&args).await?;
    report::setup(&setup, &fixture);

    warmup(&args, &setup, &queries).await;

    let mut baseline: Option<Vec<Option<U256>>> = None;
    let mut measurements = Vec::new();
    for config in matrix(&args) {
        let (measurement, values) =
            measure(config, &args, &setup, &queries, baseline.as_deref()).await;
        report::progress(&measurement);
        measurements.push(measurement);
        if baseline.is_none() {
            baseline = Some(values);
        }
    }

    report::table(&measurements);
    report::write_files(&measurements, args.json.as_deref(), args.csv.as_deref())?;
    Ok(())
}

/// The measured operation. Every number the benchmark reports about a config
/// comes from here, and nothing outside this function is timed.
async fn timed_pass(fetcher: &dyn BalanceFetching, queries: &[Query]) -> (Pass, Vec<Option<U256>>) {
    let before = Snapshot::take();
    let start = Instant::now();

    let results = fetcher.get_balances(queries).await;

    let wall_ms = start.elapsed().as_millis();
    let delta = before.delta(&Snapshot::take());

    let ok = results.iter().filter(|result| result.is_ok()).count();
    let pass = Pass {
        wall_ms,
        calls: delta.total_requests(),
        http: delta.http_requests(),
        batched: delta.total_batched(),
        unbatched: delta.unbatched(),
        batch_fill: delta.batch_fill(),
        batching: delta.batching(),
        mean_call_ms: delta.mean_request_seconds() * 1000.0,
        ok,
        err: results.len() - ok,
        methods: delta.methods(),
    };
    let values = results
        .iter()
        .map(|result| result.as_ref().ok().copied())
        .collect();

    (pass, values)
}

/// Runs every timed pass for one config. Returns the measurement and the last
/// pass's values, which become the baseline if this is the first config.
async fn measure(
    config: Config,
    args: &Args,
    setup: &Setup,
    queries: &[Query],
    baseline: Option<&[Option<U256>]>,
) -> (Measurement, Vec<Option<U256>>) {
    // A fresh provider per config, otherwise the batching parameters of the
    // previous one keep applying.
    let web3 = ethrpc::web3(config.ethrpc(), &args.node_url, Some("bench"));
    let fetcher = setup.fetcher(&web3, config.multicall_batch_size);

    // Opens the HTTP connections and resolves the one-off `Multicall3` code
    // lookup, neither of which belongs in a timed pass.
    fetcher.get_balances(&queries[..queries.len().min(2)]).await;

    let mut passes = Vec::new();
    let mut first = None;
    let mut last = Vec::new();
    for _ in 0..args.repeat {
        let (pass, values) = timed_pass(fetcher.as_ref(), queries).await;
        passes.push(pass);
        if first.is_none() {
            first = Some(values.clone());
        }
        last = values;
    }

    let (parity_mismatches, examples) = match baseline {
        Some(baseline) if !args.skip_parity => {
            let differing = mismatches(baseline, &last);
            let examples = differing
                .iter()
                .take(5)
                .map(|&index| Mismatch {
                    owner: queries[index].owner,
                    token: queries[index].token,
                    baseline: baseline[index],
                    actual: last[index],
                })
                .collect();
            (Some(differing.len()), examples)
        }
        _ => (None, Vec::new()),
    };
    let volatile = first
        .filter(|_| args.repeat > 1)
        .map(|first| mismatches(&first, &last).len());

    let measurement = Measurement {
        config,
        passes,
        parity_mismatches,
        volatile,
        examples,
    };
    (measurement, last)
}

/// Untimed passes so the node's state cache is equally warm for every config,
/// instead of the first one paying to warm it for the rest.
async fn warmup(args: &Args, setup: &Setup, queries: &[Query]) {
    if args.warmup == 0 {
        return;
    }
    let fetcher = setup.fetcher(&setup.control, WARMUP_BATCH_SIZE);
    for pass in 1..=args.warmup {
        let start = Instant::now();
        let results = fetcher.get_balances(queries).await;
        let ok = results.iter().filter(|result| result.is_ok()).count();
        report::warmup(pass, args.warmup, start.elapsed(), ok, results.len());
    }
}

/// The swept matrix, deduplicated: normalisation collapses several requested
/// combinations onto the same one, and benchmarking it twice tells us nothing.
fn matrix(args: &Args) -> Vec<Config> {
    let mut configs: Vec<Config> = Vec::new();
    for multicall in &args.multicall_batch_size {
        for batch in &args.ethrpc_batch_size {
            for concurrency in &args.ethrpc_concurrency {
                for delay in &args.ethrpc_batch_delay_ms {
                    let config = Config::new(*multicall, *batch, *concurrency, *delay);
                    if !configs.contains(&config) {
                        configs.push(config);
                    }
                }
            }
        }
    }
    configs
}
