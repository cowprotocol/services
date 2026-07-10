//! The `verify` subcommand: a read-only, forward, exhaustive cross-check of the
//! `settlements` and `trades` DB tables against the canonical chain over a
//! block range.
//!
//! Unlike `check` (which is driven by orphaned trades and so structurally
//! misses coherently-mislocated fork groups and "adopted" trades that still
//! resolve internally) this walks every block in the range and compares the DB
//! layout to the chain layout directly. It is read-only and safe against the
//! read replica; its output is meant to feed `repair`.

use {
    crate::{
        chain::{
            CanonicalSettlement,
            CanonicalTrade,
            SettlementSource,
            decode_canonical,
            fetch_logs,
            format_sources,
        },
        db::{
            DbSettlementInRange,
            DbTradeInRange,
            db_settlements_in_range,
            db_trades_in_range,
            table_stats,
            trades_have_tx_hash,
        },
        progress::{ProgressStore, VerifyRun, network_name, save_report},
    },
    alloy_primitives::{Address, B256, U256, hex},
    alloy_provider::Provider,
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    futures::stream::{self, StreamExt},
    number::conversions::u256_to_big_decimal,
    serde_json::json,
    sqlx::{Connection, PgConnection},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::Path,
    },
};

/// A single discrepancy between the DB and the chain at one block.
#[allow(clippy::enum_variant_names)]
pub enum Mismatch {
    /// A DB settlement whose (log_index) the chain has no settlement at.
    SettlementNotOnChain {
        log_index: u64,
        tx_hash: Vec<u8>,
        solver: Vec<u8>,
    },
    /// A chain settlement the DB has no row for at that log_index.
    SettlementMissing {
        log_index: u64,
        tx_hash: B256,
        solver: Address,
    },
    /// DB and chain both have a settlement at this log_index but the tx hash
    /// disagrees (the settlement was indexed from a fork).
    SettlementWrongTxHash {
        log_index: u64,
        db_tx_hash: Vec<u8>,
        chain_tx_hash: B256,
    },
    /// A settlement present on both sides resolves a different number of trades
    /// in the DB than the chain emitted for it.
    TradeCountMismatch {
        settlement_log_index: u64,
        db_count: usize,
        chain_count: usize,
    },
    /// DB and chain both have a trade at this log_index but their order uid or
    /// amounts disagree.
    TradeContentMismatch {
        log_index: u64,
        order_uid: Vec<u8>,
        diffs: Vec<String>,
    },
    /// A DB trade at a log_index the chain has no trade at (a mislocated fill).
    TradeMisplaced { log_index: u64, order_uid: Vec<u8> },
    /// DB and chain both have a trade at this log_index but the tx hash
    /// disagrees (the fill was adopted from another settlement).
    TradeWrongTxHash {
        log_index: u64,
        order_uid: Vec<u8>,
        db_tx_hash: Vec<u8>,
        chain_tx_hash: B256,
    },
}

impl Mismatch {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SettlementNotOnChain { .. } => "settlement_not_on_chain",
            Self::SettlementMissing { .. } => "settlement_missing",
            Self::SettlementWrongTxHash { .. } => "settlement_wrong_tx_hash",
            Self::TradeCountMismatch { .. } => "trade_count_mismatch",
            Self::TradeContentMismatch { .. } => "trade_content_mismatch",
            Self::TradeMisplaced { .. } => "trade_misplaced",
            Self::TradeWrongTxHash { .. } => "trade_wrong_tx_hash",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::SettlementNotOnChain {
                log_index,
                tx_hash,
                solver,
            } => format!(
                "log {log_index}: db settlement tx {} solver {} not on chain",
                hex::encode_prefixed(tx_hash),
                hex::encode_prefixed(solver),
            ),
            Self::SettlementMissing {
                log_index,
                tx_hash,
                solver,
            } => format!(
                "log {log_index}: chain settlement tx {tx_hash} solver {solver} missing from db"
            ),
            Self::SettlementWrongTxHash {
                log_index,
                db_tx_hash,
                chain_tx_hash,
            } => format!(
                "log {log_index}: settlement tx db {} != chain {chain_tx_hash}",
                hex::encode_prefixed(db_tx_hash),
            ),
            Self::TradeCountMismatch {
                settlement_log_index,
                db_count,
                chain_count,
            } => format!(
                "settlement log {settlement_log_index} resolves {db_count} db trades but chain \
                 emitted {chain_count}"
            ),
            Self::TradeContentMismatch {
                log_index,
                order_uid,
                diffs,
            } => format!(
                "log {log_index}: order {} {}",
                hex::encode_prefixed(order_uid),
                diffs.join("; ")
            ),
            Self::TradeMisplaced {
                log_index,
                order_uid,
            } => format!(
                "log {log_index}: db trade order {} has no chain trade there",
                hex::encode_prefixed(order_uid)
            ),
            Self::TradeWrongTxHash {
                log_index,
                order_uid,
                db_tx_hash,
                chain_tx_hash,
            } => format!(
                "log {log_index}: trade order {} tx db {} != chain {chain_tx_hash}",
                hex::encode_prefixed(order_uid),
                hex::encode_prefixed(db_tx_hash),
            ),
        }
    }

    fn json(&self, block: u64) -> serde_json::Value {
        let mut value = json!({ "block": block, "kind": self.kind() });
        let object = value.as_object_mut().unwrap();
        match self {
            Self::SettlementNotOnChain {
                log_index,
                tx_hash,
                solver,
            } => {
                object.insert("log_index".into(), (*log_index).into());
                object.insert("db_tx_hash".into(), hex::encode_prefixed(tx_hash).into());
                object.insert("solver".into(), hex::encode_prefixed(solver).into());
            }
            Self::SettlementMissing {
                log_index,
                tx_hash,
                solver,
            } => {
                object.insert("log_index".into(), (*log_index).into());
                object.insert("chain_tx_hash".into(), tx_hash.to_string().into());
                object.insert("solver".into(), solver.to_string().into());
            }
            Self::SettlementWrongTxHash {
                log_index,
                db_tx_hash,
                chain_tx_hash,
            } => {
                object.insert("log_index".into(), (*log_index).into());
                object.insert("db_tx_hash".into(), hex::encode_prefixed(db_tx_hash).into());
                object.insert("chain_tx_hash".into(), chain_tx_hash.to_string().into());
            }
            Self::TradeCountMismatch {
                settlement_log_index,
                db_count,
                chain_count,
            } => {
                object.insert(
                    "settlement_log_index".into(),
                    (*settlement_log_index).into(),
                );
                object.insert("db_count".into(), (*db_count).into());
                object.insert("chain_count".into(), (*chain_count).into());
            }
            Self::TradeContentMismatch {
                log_index,
                order_uid,
                diffs,
            } => {
                object.insert("log_index".into(), (*log_index).into());
                object.insert("order_uid".into(), hex::encode_prefixed(order_uid).into());
                object.insert("diffs".into(), diffs.clone().into());
            }
            Self::TradeMisplaced {
                log_index,
                order_uid,
            } => {
                object.insert("log_index".into(), (*log_index).into());
                object.insert("order_uid".into(), hex::encode_prefixed(order_uid).into());
            }
            Self::TradeWrongTxHash {
                log_index,
                order_uid,
                db_tx_hash,
                chain_tx_hash,
            } => {
                object.insert("log_index".into(), (*log_index).into());
                object.insert("order_uid".into(), hex::encode_prefixed(order_uid).into());
                object.insert("db_tx_hash".into(), hex::encode_prefixed(db_tx_hash).into());
                object.insert("chain_tx_hash".into(), chain_tx_hash.to_string().into());
            }
        }
        value
    }
}

/// Every discrepancy found at one block.
pub struct BlockReport {
    pub block: u64,
    pub mismatches: Vec<Mismatch>,
}

/// Lists the DB trade fields that differ from the on-chain Trade event
/// (excluding the tx hash, which is cross-checked separately). Empty means the
/// content matches.
fn trade_content_diffs(db: &DbTradeInRange, chain: &CanonicalTrade) -> Vec<String> {
    let mut diffs = Vec::new();
    if db.order_uid != chain.order_uid {
        diffs.push(format!(
            "order_uid chain {} != db {}",
            hex::encode_prefixed(&chain.order_uid),
            hex::encode_prefixed(&db.order_uid)
        ));
    }
    let mut check = |name: &str, chain: &U256, db_value: &BigDecimal| {
        if u256_to_big_decimal(chain) != *db_value {
            diffs.push(format!("{name} chain {chain} != db {db_value}"));
        }
    };
    check("sell_amount", &chain.sell_amount, &db.sell_amount);
    check("buy_amount", &chain.buy_amount, &db.buy_amount);
    check("fee_amount", &chain.fee_amount, &db.fee_amount);
    diffs
}

/// Counts how many trades each settlement resolves, using the same association
/// as the API (`get_trades_for_settlement`): a trade belongs to the first
/// settlement with a higher log_index in its block. Trades with no settlement
/// after them (orphans) are not counted; they surface as `TradeMisplaced`.
fn trades_per_settlement(settlement_logs: &[u64], trade_logs: &[u64]) -> BTreeMap<u64, usize> {
    let mut counts: BTreeMap<u64, usize> = settlement_logs.iter().map(|&l| (l, 0)).collect();
    for &trade in trade_logs {
        if let Some(&settlement) = settlement_logs.iter().find(|&&s| s > trade) {
            *counts.get_mut(&settlement).unwrap() += 1;
        }
    }
    counts
}

/// Compares the DB and chain layout of a single block. Inputs are the block's
/// slices of each side, already sorted by log_index.
fn compare_block(
    chain_settlements: &[&CanonicalSettlement],
    chain_trades: &[&CanonicalTrade],
    db_settlements: &[&DbSettlementInRange],
    db_trades: &[&DbTradeInRange],
) -> Vec<Mismatch> {
    let mut mismatches = Vec::new();

    // Settlements, keyed by log_index (unique within a block on both sides).
    let chain_settle: BTreeMap<u64, &CanonicalSettlement> = chain_settlements
        .iter()
        .map(|s| (s.log_index, *s))
        .collect();
    let db_settle: BTreeMap<u64, &DbSettlementInRange> = db_settlements
        .iter()
        .map(|s| (s.log_index.cast_unsigned(), *s))
        .collect();

    for (&log_index, db) in &db_settle {
        match chain_settle.get(&log_index) {
            None => mismatches.push(Mismatch::SettlementNotOnChain {
                log_index,
                tx_hash: db.tx_hash.clone(),
                solver: db.solver.clone(),
            }),
            Some(chain) if db.tx_hash.as_slice() != chain.tx_hash.as_slice() => {
                mismatches.push(Mismatch::SettlementWrongTxHash {
                    log_index,
                    db_tx_hash: db.tx_hash.clone(),
                    chain_tx_hash: chain.tx_hash,
                });
            }
            Some(_) => {}
        }
    }
    for (&log_index, chain) in &chain_settle {
        if !db_settle.contains_key(&log_index) {
            mismatches.push(Mismatch::SettlementMissing {
                log_index,
                tx_hash: chain.tx_hash,
                solver: chain.solver,
            });
        }
    }

    // Trades, keyed by log_index.
    let chain_trade: BTreeMap<u64, &CanonicalTrade> =
        chain_trades.iter().map(|t| (t.log_index, *t)).collect();
    for db in db_trades {
        let log_index = db.log_index.cast_unsigned();
        match chain_trade.get(&log_index) {
            None => mismatches.push(Mismatch::TradeMisplaced {
                log_index,
                order_uid: db.order_uid.clone(),
            }),
            Some(chain) => {
                let diffs = trade_content_diffs(db, chain);
                if !diffs.is_empty() {
                    mismatches.push(Mismatch::TradeContentMismatch {
                        log_index,
                        order_uid: db.order_uid.clone(),
                        diffs,
                    });
                }
                if let Some(db_tx) = &db.tx_hash
                    && db_tx.as_slice() != chain.tx_hash.as_slice()
                {
                    mismatches.push(Mismatch::TradeWrongTxHash {
                        log_index,
                        order_uid: db.order_uid.clone(),
                        db_tx_hash: db_tx.clone(),
                        chain_tx_hash: chain.tx_hash,
                    });
                }
            }
        }
    }

    // Per-settlement trade counts, for settlements present on both sides.
    let chain_settle_logs: Vec<u64> = chain_settle.keys().copied().collect();
    let db_settle_logs: Vec<u64> = db_settle.keys().copied().collect();
    let chain_counts = trades_per_settlement(
        &chain_settle_logs,
        &chain_trades.iter().map(|t| t.log_index).collect::<Vec<_>>(),
    );
    let db_counts = trades_per_settlement(
        &db_settle_logs,
        &db_trades
            .iter()
            .map(|t| t.log_index.cast_unsigned())
            .collect::<Vec<_>>(),
    );
    for (&log_index, &db_count) in &db_counts {
        if let Some(&chain_count) = chain_counts.get(&log_index)
            && db_count != chain_count
        {
            mismatches.push(Mismatch::TradeCountMismatch {
                settlement_log_index: log_index,
                db_count,
                chain_count,
            });
        }
    }

    mismatches
}

#[allow(clippy::too_many_arguments)]
pub async fn verify_cmd(
    provider: &impl Provider,
    sources: &[SettlementSource],
    chain_id: u64,
    db_url: &str,
    from_block: Option<u64>,
    to_block: Option<u64>,
    chunk: u64,
    concurrency: usize,
    max_mismatch_blocks: u64,
    finality: u64,
    json: bool,
    report_dir: &Path,
    progress_db: &Path,
) -> Result<()> {
    anyhow::ensure!(chunk > 0, "--chunk must be positive");
    anyhow::ensure!(concurrency > 0, "--concurrency must be positive");

    let head = provider
        .get_block_number()
        .await
        .context("could not fetch chain head")?;
    let finalized_head = head.saturating_sub(finality);

    let mut db = PgConnection::connect(db_url)
        .await
        .context("could not connect to database")?;

    // Resolve the range, defaulting to the block span of the indexed tables so
    // that omitting both bounds scans the whole database.
    let stats = table_stats(&mut db).await?;
    let db_min = [stats.min_trade_block, stats.min_settlement_block]
        .into_iter()
        .flatten()
        .min();
    let db_max = [stats.max_trade_block, stats.max_settlement_block]
        .into_iter()
        .flatten()
        .max();

    let from_block = match from_block {
        Some(from) => from,
        None => u64::try_from(db_min.context("database has no trades or settlements to verify")?)
            .context("negative block number in database")?,
    };
    let to_block = match to_block {
        Some(to) => {
            anyhow::ensure!(
                to <= finalized_head,
                "--to-block {to} reaches unfinalized blocks (chain head {head} - --finality \
                 {finality} = {finalized_head}); it may still reorg. Lower --to-block or \
                 --finality."
            );
            to
        }
        None => {
            let db_max =
                u64::try_from(db_max.context("database has no trades or settlements to verify")?)
                    .context("negative block number in database")?;
            if db_max > finalized_head {
                tracing::warn!(
                    db_max,
                    finalized_head,
                    "clamping the default --to-block to the finalized head; unfinalized blocks \
                     above it are not verified (they may still reorg)"
                );
                finalized_head
            } else {
                db_max
            }
        }
    };
    anyhow::ensure!(
        from_block <= to_block,
        "resolved range is empty: --from-block {from_block} is above --to-block {to_block} (the \
         indexed history may lie entirely within the unfinalized head)"
    );

    let have_tx_hash = trades_have_tx_hash(&mut db).await?;
    if !have_tx_hash {
        tracing::warn!(
            "trades.tx_hash column absent (pre-V112 database); skipping the trade tx-hash \
             cross-check"
        );
    }

    tracing::info!(
        chain_id,
        contract = %format_sources(sources),
        from_block,
        to_block,
        chunk,
        concurrency,
        finalized_head,
        "verifying db against chain"
    );

    let mut reports: Vec<BlockReport> = Vec::new();
    let mut blocks_scanned: u64 = 0;
    let mut truncated = false;

    // Chunk the range up front, then keep up to `concurrency` getLogs calls in
    // flight while the single-connection DB comparison consumes them in block
    // order. getLogs dominates the wall time, so overlapping its latency is the
    // win; the DB work stays serial. `buffered` preserves order, so reports and
    // the early-stop below still see blocks ascending.
    let mut ranges = Vec::new();
    let mut chunk_from = from_block;
    while chunk_from <= to_block {
        let chunk_to = chunk_from.saturating_add(chunk - 1).min(to_block);
        ranges.push((chunk_from, chunk_to));
        chunk_from = chunk_to + 1;
    }
    let mut fetches = stream::iter(ranges.into_iter().map(|(from, to)| async move {
        (from, to, fetch_logs(provider, sources, from, to).await)
    }))
    .buffered(concurrency);

    while let Some((chunk_from, chunk_to, logs)) = fetches.next().await {
        tracing::info!(from = chunk_from, to = chunk_to, "verified chunk");
        let canonical = decode_canonical(&logs?);
        let db_settlements = db_settlements_in_range(&mut db, chunk_from, chunk_to).await?;
        let db_trades = db_trades_in_range(&mut db, chunk_from, chunk_to, have_tx_hash).await?;

        // Group each side by block. All four inputs are already sorted by
        // (block, log_index).
        let mut chain_settle_by_block: BTreeMap<u64, Vec<&CanonicalSettlement>> = BTreeMap::new();
        for s in &canonical.settlements {
            chain_settle_by_block.entry(s.block).or_default().push(s);
        }
        let mut chain_trade_by_block: BTreeMap<u64, Vec<&CanonicalTrade>> = BTreeMap::new();
        for t in &canonical.trades {
            chain_trade_by_block.entry(t.block).or_default().push(t);
        }
        let mut db_settle_by_block: BTreeMap<u64, Vec<&DbSettlementInRange>> = BTreeMap::new();
        for s in &db_settlements {
            db_settle_by_block
                .entry(s.block_number.cast_unsigned())
                .or_default()
                .push(s);
        }
        let mut db_trade_by_block: BTreeMap<u64, Vec<&DbTradeInRange>> = BTreeMap::new();
        for t in &db_trades {
            db_trade_by_block
                .entry(t.block_number.cast_unsigned())
                .or_default()
                .push(t);
        }

        let blocks: BTreeSet<u64> = chain_settle_by_block
            .keys()
            .chain(chain_trade_by_block.keys())
            .chain(db_settle_by_block.keys())
            .chain(db_trade_by_block.keys())
            .copied()
            .collect();
        for block in blocks {
            let empty_settle_c: Vec<&CanonicalSettlement> = Vec::new();
            let empty_trade_c: Vec<&CanonicalTrade> = Vec::new();
            let empty_settle_d: Vec<&DbSettlementInRange> = Vec::new();
            let empty_trade_d: Vec<&DbTradeInRange> = Vec::new();
            let mismatches = compare_block(
                chain_settle_by_block.get(&block).unwrap_or(&empty_settle_c),
                chain_trade_by_block.get(&block).unwrap_or(&empty_trade_c),
                db_settle_by_block.get(&block).unwrap_or(&empty_settle_d),
                db_trade_by_block.get(&block).unwrap_or(&empty_trade_d),
            );
            if !mismatches.is_empty() {
                reports.push(BlockReport { block, mismatches });
            }
        }

        blocks_scanned += chunk_to - chunk_from + 1;

        if reports.len() as u64 > max_mismatch_blocks {
            tracing::error!(
                mismatch_blocks = reports.len(),
                max_mismatch_blocks,
                "too many mismatching blocks; stopping the scan early. This usually means a wrong \
                 database or contract rather than genuine damage. Raise --max-mismatch-blocks to \
                 continue."
            );
            truncated = true;
            break;
        }
    }

    let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
    for report in &reports {
        for mismatch in &report.mismatches {
            *by_kind.entry(mismatch.kind()).or_default() += 1;
        }
    }
    let total: usize = by_kind.values().sum();

    let network = network_name(chain_id);
    let doc = json!({
        "network": network,
        "chain_id": chain_id,
        "contracts": sources.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "from": from_block,
        "to": to_block,
        "blocks_scanned": blocks_scanned,
        "truncated": truncated,
        "mismatches": reports.iter().flat_map(|r| {
            r.mismatches.iter().map(|m| m.json(r.block))
        }).collect::<Vec<_>>(),
        "summary": {
            "total": total,
            "mismatch_blocks": reports.len(),
            "by_kind": by_kind,
        },
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else if reports.is_empty() {
        println!(
            "no mismatches in blocks {from_block}..={to_block} ({blocks_scanned} scanned); the db \
             matches the chain over this range"
        );
    } else {
        println!("{:<12}  {:<24}  detail", "block", "kind");
        println!("{}  {}  {}", "-".repeat(12), "-".repeat(24), "-".repeat(6));
        for report in &reports {
            for mismatch in &report.mismatches {
                println!(
                    "{:<12}  {:<24}  {}",
                    report.block,
                    mismatch.kind(),
                    mismatch.detail()
                );
            }
        }
        println!(
            "\nsummary: {total} mismatches across {} blocks (scanned {blocks_scanned} of \
             {from_block}..={to_block})",
            reports.len()
        );
        for (kind, count) in &by_kind {
            println!("  {kind}: {count}");
        }
    }

    // Persist the report to disk and advance this environment's progress before
    // the mismatch exit below, so a failed verification is still recorded.
    let report_path = save_report(report_dir, &network, &doc)?;
    tracing::info!(path = %report_path.display(), "saved report");

    let mut store = ProgressStore::open(progress_db).await?;
    store
        .record_run(&VerifyRun {
            network,
            chain_id,
            db_url: db_url.to_owned(),
            from_block,
            to_block,
            blocks_scanned,
            mismatch_blocks: reports.len() as u64,
            mismatches: total as u64,
            truncated,
            report_path: Some(report_path.display().to_string()),
        })
        .await?;
    tracing::info!(path = %progress_db.display(), "recorded progress");

    tracing::info!(
        from_block,
        to_block,
        blocks_scanned,
        mismatch_blocks = reports.len(),
        mismatches = total,
        truncated,
        "verify complete (only the scanned range was checked, not the whole db)"
    );

    if total > 0 {
        std::process::exit(1);
    }
    Ok(())
}
