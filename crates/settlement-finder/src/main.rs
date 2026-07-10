mod chain;
mod commands;
mod db;
mod filter;
mod orphans;
mod progress;
mod verify;

use {
    crate::{
        chain::{SettlementSource, parse_settlement_source},
        commands::{
            backfill::backfill_cmd,
            block::block_cmd,
            check::check_cmd,
            repair::repair_cmd,
            stats::stats_cmd,
        },
        filter::FilterArgs,
        verify::verify_cmd,
    },
    alloy_provider::Provider,
    anyhow::{Context, Result},
    clap::{Parser, Subcommand},
    contracts::GPv2Settlement,
};

/// Cross-checks the `settlements` and `trades` DB tables against the chain
/// and maintains the trades.tx_hash column (migration V112).
///
/// Progress and warnings go to stderr, results to stdout.
#[derive(Parser)]
struct Args {
    /// RPC endpoint of the chain to inspect.
    #[arg(long, env = "RPC_URL", global = true)]
    rpc_url: Option<String>,

    /// Address of the settlement contract, optionally restricted to the block
    /// window it was active in with a `:FROM-TO` suffix (either side may be
    /// empty for an open end). Repeatable: pass more than once to scan several
    /// deployments together, e.g. a whole-database `verify` spanning the
    /// mainnet contract migration:
    /// `--settlement 0x3328f5f2cEcAF00a2443082B657CedEAf70bfAEf:-12500000
    ///  --settlement 0x9008D19f58AAbD9eD0D60971565AA8510560ab41:12500000-`.
    /// Defaults to the canonical deployment on the connected chain, unbounded.
    #[arg(long, global = true, value_parser = parse_settlement_source)]
    settlement: Vec<SettlementSource>,

    /// Log filter directives (see tracing_subscriber's EnvFilter). Logs go to
    /// stderr; results go to stdout.
    #[arg(long, global = true, default_value = "info")]
    log_filter: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the settlement transactions of a block. If the block (or none of
    /// the given trade filters) matches, brute-force searches the neighboring
    /// blocks. Exits with 1 if nothing is found.
    Block {
        /// Block number to inspect.
        block: u64,

        /// How many blocks before and after the target block to search when
        /// the target block itself contains no match. 0 disables the search.
        #[arg(long, default_value_t = 25)]
        window: u64,

        #[command(flatten)]
        filter: FilterArgs,
    },
    /// Find DB trades that no settlement event resolves to (the association
    /// used by the backfill subcommand) and locate each of them on-chain: first
    /// in the block the DB recorded, then in the neighboring blocks. An event
    /// only counts as a match if its order uid, amounts and tokens (from the
    /// orders/jit_orders tables) equal the DB data. For every match it also
    /// reports whether the settlement transaction is already indexed in the
    /// settlements table (looked up by tx hash, which reorgs preserve) and
    /// whether a trades row already exists at the located coordinates, i.e.
    /// whether the DB merely points at the wrong place. Exits with 1 if any
    /// trade could not be uniquely located.
    Check {
        /// Postgres connection string of the database to cross-check.
        #[arg(long, env = "DB_URL")]
        db: String,

        /// How many blocks before and after the recorded block to search when
        /// the recorded block itself contains no match. 0 disables the search.
        #[arg(long, default_value_t = 25)]
        window: u64,

        /// Abort before any RPC call if more than this many distinct blocks
        /// have orphaned trades (guards against an empty or wrong database).
        #[arg(long, default_value_t = 1000)]
        max_orphan_blocks: u64,
    },
    /// Re-index the block ranges around orphaned trades from canonical chain
    /// data: for every uniquely located trade (see check) the range spanning
    /// the recorded block, the located block and any mislocated settlements
    /// rows of the same tx hash is wiped (trades, settlements, invalidations,
    /// presignature_events) and re-inserted from the chain logs, carrying
    /// auction_id/solution_uid over by tx hash. jit_orders rows are left
    /// untouched (not reconstructible from events; they are joined by uid).
    /// By default rehearses the re-index in a transaction and rolls it back
    /// (needs a write connection and takes row locks); only --apply commits.
    /// Exits with 1 if any orphaned trade had to be skipped (ambiguous or not
    /// found).
    Repair {
        /// Postgres connection string of the database to repair.
        #[arg(long, env = "DB_URL")]
        db: String,

        /// How many blocks before and after the recorded block to search when
        /// the recorded block itself contains no match. 0 disables the search.
        #[arg(long, default_value_t = 25)]
        window: u64,

        /// Abort before any RPC call if more than this many distinct blocks
        /// have orphaned trades (guards against an empty or wrong database).
        #[arg(long, default_value_t = 1000)]
        max_orphan_blocks: u64,

        /// Refuse to touch ranges whose upper end is within this many blocks of
        /// the chain head, so only finalized history is re-indexed.
        #[arg(long, default_value_t = 64)]
        finality: u64,

        /// Commit the repair instead of rehearsing it in a rolled-back
        /// transaction.
        #[arg(long)]
        apply: bool,
    },
    /// Forward, exhaustive cross-check of a block range: walk every block and
    /// compare the DB's settlements/trades layout against the canonical chain.
    /// Unlike check (which starts from orphaned trades) this also catches
    /// coherently-mislocated fork groups and adopted trades that still resolve
    /// internally, so its output is the natural input to repair. Read-only;
    /// safe against the read replica. Refuses ranges reaching unfinalized
    /// blocks. Exits with 1 if any mismatch is found.
    Verify {
        /// Postgres connection string of the database to verify.
        #[arg(long, env = "DB_URL")]
        db: String,

        /// First block of the range to check (inclusive). Defaults to resuming
        /// after the last block verified for this (network, database) as
        /// recorded in the progress database, or the lowest block present in
        /// the trades/settlements tables if there is no prior run.
        #[arg(long)]
        from_block: Option<u64>,

        /// Last block of the range to check (inclusive). Defaults to the
        /// highest indexed block, clamped down to the finalized head. Together
        /// with the default --from-block this scans the whole database.
        #[arg(long)]
        to_block: Option<u64>,

        /// How many blocks to fetch and compare per getLogs call. If the node
        /// rejects a chunk as too large (too many logs / too wide a range) it
        /// is transparently split and retried, so this can be raised freely;
        /// it only sets the starting granularity.
        #[arg(long, default_value_t = 5000)]
        chunk: u64,

        /// How many chunks to fetch from the node concurrently. Higher is
        /// faster on a dedicated node; lower is safer against a rate-limited
        /// provider (failed requests are retried with backoff regardless).
        #[arg(long, default_value_t = 4)]
        concurrency: usize,

        /// Stop the scan once this many distinct blocks have mismatches (guards
        /// against a wrong database or contract rather than genuine damage).
        #[arg(long, default_value_t = 10000)]
        max_mismatch_blocks: u64,

        /// Refuse to check blocks within this many blocks of the chain head, so
        /// only finalized history (which cannot reorg) is verified.
        #[arg(long, default_value_t = 64)]
        finality: u64,

        /// Directory to save the final JSON report in. The file is named
        /// `<network>_<timestamp>.json`. Created if it does not exist.
        #[arg(long, default_value = "reports")]
        report_dir: std::path::PathBuf,

        /// SQLite database tracking verification progress per network and
        /// database URL (different databases are different environments, so
        /// their progress is kept separately). Created if it does not exist.
        #[arg(long, default_value = "settlement-finder-progress.sqlite")]
        progress_db: std::path::PathBuf,
    },
    /// Report how consistent the trades <-> settlements association is:
    /// row counts, indexed block ranges, trades that no settlement event
    /// resolves to (the rows backfill would leave NULL) and settlements that
    /// no trade resolves to (not necessarily a gap: settle() calls with an
    /// empty trades array legitimately emit no Trade events). Read-only; safe
    /// against the read replica. Needs no RPC endpoint.
    Stats {
        /// Postgres connection string of the database to inspect.
        #[arg(long, env = "DB_URL")]
        db: String,
    },
    /// Backfill trades.tx_hash (introduced in migration V112) for rows
    /// indexed before the column existed, from the settlements table. Runs in
    /// batches over the primary key, committing after each one, so it never
    /// holds long locks and can be aborted and re-run at any time. Trades
    /// without a matching settlements row are left NULL; run stats beforehand
    /// to gauge how many to expect and check/repair to fix them. Dry run
    /// (default) only reports counts; --apply executes. Needs no RPC endpoint.
    Backfill {
        /// Postgres connection string of the database to backfill.
        #[arg(long, env = "DB_URL")]
        db: String,

        /// How many trades rows to update per transaction.
        #[arg(long, default_value_t = 50000)]
        batch_size: i64,

        /// Execute the backfill instead of reporting counts.
        #[arg(long)]
        apply: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Route all logs to stderr so stdout carries only results.
    observe::tracing::init::initialize(&observe::Config::new(
        &args.log_filter,
        Some(tracing::Level::TRACE),
        false,
        None,
    ));

    // The DB-only commands do not need a chain connection.
    match args.command {
        Command::Stats { db } => return stats_cmd(&db).await,
        Command::Backfill {
            db,
            batch_size,
            apply,
        } => return backfill_cmd(&db, batch_size, apply).await,
        _ => (),
    }

    let rpc_url = args
        .rpc_url
        .context("missing RPC endpoint: pass --rpc-url or set RPC_URL")?;
    let (provider, _wallet) = ethrpc::alloy::unbuffered_provider(&rpc_url, None);

    let chain_id = provider
        .get_chain_id()
        .await
        .context("could not fetch chain id")?;
    let sources = if args.settlement.is_empty() {
        vec![SettlementSource {
            address: GPv2Settlement::deployment_address(&chain_id).with_context(|| {
                format!("no known settlement deployment for chain {chain_id}, pass --settlement")
            })?,
            from_block: None,
            to_block: None,
        }]
    } else {
        args.settlement
    };
    let sources = sources.as_slice();

    match args.command {
        Command::Block {
            block,
            window,
            filter,
        } => block_cmd(&provider, sources, chain_id, block, window, &filter).await,
        Command::Check {
            db,
            window,
            max_orphan_blocks,
        } => check_cmd(&provider, sources, chain_id, &db, window, max_orphan_blocks).await,
        Command::Repair {
            db,
            window,
            max_orphan_blocks,
            finality,
            apply,
        } => {
            repair_cmd(
                &provider,
                sources,
                chain_id,
                &db,
                window,
                max_orphan_blocks,
                finality,
                apply,
            )
            .await
        }
        Command::Verify {
            db,
            from_block,
            to_block,
            chunk,
            concurrency,
            max_mismatch_blocks,
            finality,
            report_dir,
            progress_db,
        } => {
            verify_cmd(
                &provider,
                sources,
                chain_id,
                &rpc_url,
                &db,
                from_block,
                to_block,
                chunk,
                concurrency,
                max_mismatch_blocks,
                finality,
                &report_dir,
                &progress_db,
            )
            .await
        }
        Command::Stats { .. } | Command::Backfill { .. } => unreachable!("handled above"),
    }
}
