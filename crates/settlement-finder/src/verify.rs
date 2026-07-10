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
            canonical_block_hash,
            decode_canonical,
            fetch_logs,
            fetch_logs_by_block_hash,
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
        progress::{ProgressStore, VerifyRun, network_name, sanitize_rpc_url, save_report},
    },
    alloy_primitives::{Address, Bytes, TxHash, U256, hex},
    alloy_provider::Provider,
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    futures::stream::{self, StreamExt},
    number::conversions::u256_to_big_decimal,
    serde::Serialize,
    serde_json::json,
    sqlx::{Connection, PgConnection},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::Path,
    },
};

/// A single discrepancy between the DB and the chain at one block. Serializes
/// to a flat object tagged with `kind` (each field's name is its JSON key, and
/// `Bytes` fields render as `0x`-prefixed hex); the enclosing block number is
/// added by [`Mismatch::json`].
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum Mismatch {
    /// A DB settlement whose (log_index) the chain has no settlement at.
    SettlementNotOnChain {
        log_index: u64,
        db_tx_hash: Bytes,
        solver: Address,
    },
    /// A chain settlement the DB has no row for at that log_index.
    SettlementMissing {
        log_index: u64,
        chain_tx_hash: TxHash,
        solver: Address,
    },
    /// DB and chain both have a settlement at this log_index but the tx hash
    /// disagrees (the settlement was indexed from a fork).
    SettlementWrongTxHash {
        log_index: u64,
        db_tx_hash: Bytes,
        chain_tx_hash: TxHash,
    },
    /// DB and chain have the same settlement (by tx hash) in this block but at
    /// different log indices. Reported as one finding instead of a
    /// not-on-chain/missing pair so an index shift reads as what it is.
    SettlementWrongLogIndex {
        db_log_index: u64,
        chain_log_index: u64,
        tx_hash: Bytes,
    },
    /// DB and chain both have this settlement at this log_index (same tx hash)
    /// but the recorded solver differs.
    SettlementSolverMismatch {
        log_index: u64,
        db_solver: Address,
        chain_solver: Address,
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
        order_uid: Bytes,
        diffs: Vec<String>,
    },
    /// A DB trade at a log_index the chain has no trade at (a mislocated fill).
    TradeMisplaced { log_index: u64, order_uid: Bytes },
    /// DB and chain have the same trade (by order uid and amounts) in this
    /// block but at different log indices.
    TradeWrongLogIndex {
        db_log_index: u64,
        chain_log_index: u64,
        order_uid: Bytes,
    },
    /// A chain trade at a log_index the DB has no trade at.
    TradeMissing {
        log_index: u64,
        order_uid: Bytes,
        chain_tx_hash: TxHash,
    },
    /// DB and chain both have a trade at this log_index but the tx hash
    /// disagrees (the fill was adopted from another settlement).
    TradeWrongTxHash {
        log_index: u64,
        order_uid: Bytes,
        db_tx_hash: Bytes,
        chain_tx_hash: TxHash,
    },
}

impl Mismatch {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SettlementNotOnChain { .. } => "settlement_not_on_chain",
            Self::SettlementMissing { .. } => "settlement_missing",
            Self::SettlementWrongTxHash { .. } => "settlement_wrong_tx_hash",
            Self::SettlementWrongLogIndex { .. } => "settlement_wrong_log_index",
            Self::SettlementSolverMismatch { .. } => "settlement_solver_mismatch",
            Self::TradeCountMismatch { .. } => "trade_count_mismatch",
            Self::TradeContentMismatch { .. } => "trade_content_mismatch",
            Self::TradeMisplaced { .. } => "trade_misplaced",
            Self::TradeWrongLogIndex { .. } => "trade_wrong_log_index",
            Self::TradeMissing { .. } => "trade_missing",
            Self::TradeWrongTxHash { .. } => "trade_wrong_tx_hash",
        }
    }

    /// The mismatch as a flat JSON object: the enclosing `block` plus the
    /// derived `{ kind, <fields> }` representation, i.e. `{ block, ...self }`.
    fn json(&self, block: u64) -> serde_json::Value {
        #[derive(Serialize)]
        struct WithBlock<'a> {
            block: u64,
            #[serde(flatten)]
            mismatch: &'a Mismatch,
        }
        serde_json::to_value(WithBlock {
            block,
            mismatch: self,
        })
        .expect("Mismatch serializes to a JSON object")
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

    // Chain settlements paired to an unmatched DB settlement by tx hash, so a
    // log-index shift is reported once as such rather than as an alarming
    // not-on-chain/missing pair.
    let mut claimed_settlements: BTreeSet<u64> = BTreeSet::new();
    for (&log_index, db) in &db_settle {
        match chain_settle.get(&log_index) {
            None => {
                let moved = chain_settle.iter().find(|(chain_index, chain)| {
                    !db_settle.contains_key(chain_index)
                        && !claimed_settlements.contains(chain_index)
                        && chain.tx_hash.as_slice() == db.tx_hash.as_slice()
                });
                match moved {
                    Some((&chain_log_index, _)) => {
                        claimed_settlements.insert(chain_log_index);
                        mismatches.push(Mismatch::SettlementWrongLogIndex {
                            db_log_index: log_index,
                            chain_log_index,
                            tx_hash: db.tx_hash.clone().into(),
                        });
                    }
                    None => mismatches.push(Mismatch::SettlementNotOnChain {
                        log_index,
                        db_tx_hash: db.tx_hash.clone().into(),
                        solver: Address::from_slice(&db.solver),
                    }),
                }
            }
            Some(chain) if db.tx_hash.as_slice() != chain.tx_hash.as_slice() => {
                mismatches.push(Mismatch::SettlementWrongTxHash {
                    log_index,
                    db_tx_hash: db.tx_hash.clone().into(),
                    chain_tx_hash: chain.tx_hash,
                });
            }
            Some(chain) if db.solver.as_slice() != chain.solver.as_slice() => {
                mismatches.push(Mismatch::SettlementSolverMismatch {
                    log_index,
                    db_solver: Address::from_slice(&db.solver),
                    chain_solver: chain.solver,
                });
            }
            Some(_) => {}
        }
    }
    for (&log_index, chain) in &chain_settle {
        if !db_settle.contains_key(&log_index) && !claimed_settlements.contains(&log_index) {
            mismatches.push(Mismatch::SettlementMissing {
                log_index,
                chain_tx_hash: chain.tx_hash,
                solver: chain.solver,
            });
        }
    }

    // Trades, keyed by log_index, with the same moved-within-the-block pairing
    // (by order uid and amounts) as settlements.
    let chain_trade: BTreeMap<u64, &CanonicalTrade> =
        chain_trades.iter().map(|t| (t.log_index, *t)).collect();
    let db_trade_indices: BTreeSet<u64> = db_trades
        .iter()
        .map(|t| t.log_index.cast_unsigned())
        .collect();
    let mut claimed_trades: BTreeSet<u64> = BTreeSet::new();
    for db in db_trades {
        let log_index = db.log_index.cast_unsigned();
        match chain_trade.get(&log_index) {
            None => {
                let moved = chain_trade.iter().find(|(chain_index, chain)| {
                    !db_trade_indices.contains(chain_index)
                        && !claimed_trades.contains(chain_index)
                        && trade_content_diffs(db, chain).is_empty()
                });
                match moved {
                    Some((&chain_log_index, _)) => {
                        claimed_trades.insert(chain_log_index);
                        mismatches.push(Mismatch::TradeWrongLogIndex {
                            db_log_index: log_index,
                            chain_log_index,
                            order_uid: db.order_uid.clone().into(),
                        });
                    }
                    None => mismatches.push(Mismatch::TradeMisplaced {
                        log_index,
                        order_uid: db.order_uid.clone().into(),
                    }),
                }
            }
            Some(chain) => {
                let diffs = trade_content_diffs(db, chain);
                if !diffs.is_empty() {
                    mismatches.push(Mismatch::TradeContentMismatch {
                        log_index,
                        order_uid: db.order_uid.clone().into(),
                        diffs,
                    });
                }
                if let Some(db_tx) = &db.tx_hash
                    && db_tx.as_slice() != chain.tx_hash.as_slice()
                {
                    mismatches.push(Mismatch::TradeWrongTxHash {
                        log_index,
                        order_uid: db.order_uid.clone().into(),
                        db_tx_hash: db_tx.clone().into(),
                        chain_tx_hash: chain.tx_hash,
                    });
                }
            }
        }
    }
    for (&log_index, chain) in &chain_trade {
        if !db_trade_indices.contains(&log_index) && !claimed_trades.contains(&log_index) {
            mismatches.push(Mismatch::TradeMissing {
                log_index,
                order_uid: chain.order_uid.clone().into(),
                chain_tx_hash: chain.tx_hash,
            });
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

/// Re-checks a mismatching block against a blockHash-pinned getLogs query
/// (EIP-234) before reporting it. A node's block-number log index can serve
/// another block's logs under the queried number — observed on public Gnosis
/// endpoints as shifted log indices producing mass false mismatches — while
/// the same node answers correctly when asked by hash, so mismatches that
/// vanish here were RPC artifacts, not DB damage. Falls back to the
/// unvalidated mismatches (with a warning) if the re-check itself fails.
async fn revalidate_block(
    provider: &impl Provider,
    sources: &[SettlementSource],
    block: u64,
    db_settlements: &[&DbSettlementInRange],
    db_trades: &[&DbTradeInRange],
    unvalidated: Vec<Mismatch>,
) -> Vec<Mismatch> {
    let recheck = async {
        let hash = canonical_block_hash(provider, block)
            .await?
            .with_context(|| format!("node has no block {block}"))?;
        let logs = fetch_logs_by_block_hash(provider, sources, block, hash).await?;
        let canonical = decode_canonical(&logs);
        anyhow::Ok(compare_block(
            &canonical.settlements.iter().collect::<Vec<_>>(),
            &canonical.trades.iter().collect::<Vec<_>>(),
            db_settlements,
            db_trades,
        ))
    };
    match recheck.await {
        Ok(confirmed) => {
            if confirmed.len() != unvalidated.len() {
                tracing::warn!(
                    block,
                    unvalidated = unvalidated.len(),
                    confirmed = confirmed.len(),
                    "range getLogs and blockHash getLogs disagree; the node's block-number log \
                     index is unreliable, trusting the blockHash view"
                );
            }
            confirmed
        }
        Err(err) => {
            tracing::warn!(
                block,
                ?err,
                "could not re-check the block by hash; reporting the unvalidated mismatches"
            );
            unvalidated
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn verify_cmd(
    provider: &impl Provider,
    sources: &[SettlementSource],
    chain_id: u64,
    rpc_url: &str,
    db_url: &str,
    from_block: Option<u64>,
    to_block: Option<u64>,
    chunk: u64,
    concurrency: usize,
    max_mismatch_blocks: u64,
    finality: u64,
    report_dir: &Path,
    progress_db: &Path,
) -> Result<()> {
    anyhow::ensure!(chunk > 0, "--chunk must be positive");
    anyhow::ensure!(concurrency > 0, "--concurrency must be positive");
    anyhow::ensure!(
        max_mismatch_blocks > 0,
        "--max-mismatch-blocks must be positive"
    );

    let head = provider
        .get_block_number()
        .await
        .context("could not fetch chain head")?;
    let finalized_head = head.saturating_sub(finality);

    let mut db = PgConnection::connect(db_url)
        .await
        .context("could not connect to database")?;

    // Opened up front so the recorded high-water mark can seed the default
    // --from-block below; reused at the end to record this run.
    let mut store = ProgressStore::open(progress_db).await?;

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
        None => {
            let db_min =
                u64::try_from(db_min.context("database has no trades or settlements to verify")?)
                    .context("negative block number in database")?;
            // Resume after the last block verified for this (network, db), so a
            // re-run continues where the previous one left off instead of
            // rescanning the whole history. Never start below db_min.
            match store.verified_to_block(chain_id, db_url).await? {
                Some(last) if last + 1 > db_min => {
                    let resume = last + 1;
                    tracing::info!(
                        resume,
                        last_verified = last,
                        "resuming after the last verified block recorded for this network and \
                         database (pass --from-block to override)"
                    );
                    resume
                }
                _ => db_min,
            }
        }
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
        "resolved range is empty: --from-block {from_block} is above --to-block {to_block} (this \
         network and database may already be verified up to the finalized head, or the indexed \
         history may lie entirely within the unfinalized head)"
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

    let total_blocks = to_block - from_block + 1;
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
        // A chunk that failed even after retries must not discard the hours of
        // work before it: stop early, save the partial report and keep the
        // progress high-water mark where it is (truncated runs don't advance).
        let logs = match logs {
            Ok(logs) => logs,
            Err(err) => {
                tracing::error!(
                    from = chunk_from,
                    to = chunk_to,
                    ?err,
                    "chunk failed after retries; stopping the scan early and saving partial \
                     results"
                );
                truncated = true;
                break;
            }
        };
        let canonical = decode_canonical(&logs);
        // A mid-scan DB error is treated like a failed chunk: stop and save what
        // was compared so far rather than discarding the whole run.
        let db_events = async {
            let settlements = db_settlements_in_range(&mut db, chunk_from, chunk_to).await?;
            let trades = db_trades_in_range(&mut db, chunk_from, chunk_to, have_tx_hash).await?;
            anyhow::Ok((settlements, trades))
        }
        .await;
        let (db_settlements, db_trades) = match db_events {
            Ok(events) => events,
            Err(err) => {
                tracing::error!(
                    from = chunk_from,
                    to = chunk_to,
                    ?err,
                    "db query failed; stopping the scan early and saving partial results"
                );
                truncated = true;
                break;
            }
        };

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
            let db_settlements = db_settle_by_block.get(&block).unwrap_or(&empty_settle_d);
            let db_trades = db_trade_by_block.get(&block).unwrap_or(&empty_trade_d);
            let mut mismatches = compare_block(
                chain_settle_by_block.get(&block).unwrap_or(&empty_settle_c),
                chain_trade_by_block.get(&block).unwrap_or(&empty_trade_c),
                db_settlements,
                db_trades,
            );
            if !mismatches.is_empty() {
                mismatches = revalidate_block(
                    provider,
                    sources,
                    block,
                    db_settlements,
                    db_trades,
                    mismatches,
                )
                .await;
            }
            if !mismatches.is_empty() {
                reports.push(BlockReport { block, mismatches });
            }
        }

        blocks_scanned += chunk_to - chunk_from + 1;
        tracing::info!(
            through_block = chunk_to,
            blocks_scanned,
            total_blocks,
            percent = blocks_scanned * 100 / total_blocks,
            mismatch_blocks = reports.len(),
            "scan progress"
        );

        if reports.len() as u64 >= max_mismatch_blocks {
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
    let rpc = sanitize_rpc_url(rpc_url);
    let doc = json!({
        "network": network,
        "chain_id": chain_id,
        "rpc": rpc,
        "contracts": sources.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "from": from_block,
        "to": to_block,
        // Scanning proceeds in block order, so a truncated run has still
        // covered everything up to here.
        "scanned_through": (blocks_scanned > 0).then(|| from_block + blocks_scanned - 1),
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

    // Always persist the report and progress first — before the stdout
    // formatting below and before the mismatch exit — so the run is recorded no
    // matter how it ends (clean, mismatched, or stopped early).
    let report_path = save_report(report_dir, &network, &doc)?;
    tracing::info!(path = %report_path.display(), "saved report");

    store
        .record_run(&VerifyRun {
            network,
            chain_id,
            rpc_url: rpc,
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

    println!("{}", serde_json::to_string_pretty(&doc)?);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn chain_settlement(log_index: u64, tx: u8, solver: u8) -> CanonicalSettlement {
        CanonicalSettlement {
            block: 1,
            log_index,
            solver: Address::repeat_byte(solver),
            tx_hash: TxHash::repeat_byte(tx),
        }
    }

    fn db_settlement(log_index: i64, tx: u8, solver: u8) -> DbSettlementInRange {
        DbSettlementInRange {
            block_number: 1,
            log_index,
            solver: Address::repeat_byte(solver).as_slice().to_vec(),
            tx_hash: TxHash::repeat_byte(tx).to_vec(),
        }
    }

    fn chain_trade(log_index: u64, uid: u8, tx: u8) -> CanonicalTrade {
        CanonicalTrade {
            block: 1,
            log_index,
            order_uid: vec![uid; 56],
            sell_amount: U256::from(1),
            buy_amount: U256::from(2),
            fee_amount: U256::from(3),
            tx_hash: TxHash::repeat_byte(tx),
        }
    }

    fn db_trade(log_index: i64, uid: u8, tx: u8) -> DbTradeInRange {
        DbTradeInRange {
            block_number: 1,
            log_index,
            order_uid: vec![uid; 56],
            sell_amount: u256_to_big_decimal(&U256::from(1)),
            buy_amount: u256_to_big_decimal(&U256::from(2)),
            fee_amount: u256_to_big_decimal(&U256::from(3)),
            tx_hash: Some(TxHash::repeat_byte(tx).to_vec()),
        }
    }

    fn compare(
        chain_settlements: &[CanonicalSettlement],
        chain_trades: &[CanonicalTrade],
        db_settlements: &[DbSettlementInRange],
        db_trades: &[DbTradeInRange],
    ) -> Vec<&'static str> {
        compare_block(
            &chain_settlements.iter().collect::<Vec<_>>(),
            &chain_trades.iter().collect::<Vec<_>>(),
            &db_settlements.iter().collect::<Vec<_>>(),
            &db_trades.iter().collect::<Vec<_>>(),
        )
        .iter()
        .map(Mismatch::kind)
        .collect()
    }

    #[test]
    fn matching_block_has_no_mismatches() {
        let kinds = compare(
            &[chain_settlement(10, 1, 1)],
            &[chain_trade(8, 7, 1), chain_trade(9, 8, 1)],
            &[db_settlement(10, 1, 1)],
            &[db_trade(8, 7, 1), db_trade(9, 8, 1)],
        );
        assert!(kinds.is_empty(), "{kinds:?}");
    }

    #[test]
    fn json_is_flat_tagged_and_lowercase_hex() {
        // Address must serialize as lowercase hex (not EIP-55 checksummed) and
        // block/kind/fields must be flattened into one object.
        let settlement = Mismatch::SettlementMissing {
            log_index: 3,
            chain_tx_hash: TxHash::repeat_byte(0xab),
            solver: Address::repeat_byte(0xcd),
        };
        assert_eq!(
            settlement.json(100),
            json!({
                "block": 100,
                "kind": "settlement_missing",
                "log_index": 3,
                "chain_tx_hash": TxHash::repeat_byte(0xab).to_string(),
                "solver": hex::encode_prefixed(Address::repeat_byte(0xcd).as_slice()),
            })
        );

        let trade = Mismatch::TradeWrongTxHash {
            log_index: 5,
            order_uid: vec![1u8; 56].into(),
            db_tx_hash: vec![2u8; 32].into(),
            chain_tx_hash: TxHash::repeat_byte(3),
        };
        assert_eq!(
            trade.json(7),
            json!({
                "block": 7,
                "kind": "trade_wrong_tx_hash",
                "log_index": 5,
                "order_uid": hex::encode_prefixed([1u8; 56]),
                "db_tx_hash": hex::encode_prefixed([2u8; 32]),
                "chain_tx_hash": TxHash::repeat_byte(3).to_string(),
            })
        );
    }

    #[test]
    fn index_shift_pairs_by_identity_instead_of_alarming() {
        // The same settlement and trades, with the chain view 20 log indices
        // lower — the exact shape a corrupt block-number log index produced on
        // Gnosis (db 138/91/92 vs chain 118/71/72). Keyed matching alone would
        // report five mismatches (not-on-chain + missing + 2x misplaced + the
        // trades missing); identity pairing reports the three real movements.
        let kinds = compare(
            &[chain_settlement(118, 1, 1)],
            &[chain_trade(71, 7, 1), chain_trade(72, 8, 1)],
            &[db_settlement(138, 1, 1)],
            &[db_trade(91, 7, 1), db_trade(92, 8, 1)],
        );
        assert_eq!(
            kinds,
            vec![
                "settlement_wrong_log_index",
                "trade_wrong_log_index",
                "trade_wrong_log_index",
            ],
        );
    }

    #[test]
    fn solver_mismatch_is_reported() {
        let kinds = compare(
            &[chain_settlement(10, 1, 1)],
            &[],
            &[db_settlement(10, 1, 2)],
            &[],
        );
        assert_eq!(kinds, vec!["settlement_solver_mismatch"]);
    }

    #[test]
    fn chain_trade_absent_from_db_is_reported() {
        let kinds = compare(
            &[chain_settlement(10, 1, 1)],
            &[chain_trade(8, 7, 1), chain_trade(9, 8, 1)],
            &[db_settlement(10, 1, 1)],
            &[db_trade(8, 7, 1)],
        );
        assert_eq!(kinds, vec!["trade_missing", "trade_count_mismatch"]);
    }

    #[test]
    fn unrelated_settlements_still_report_as_a_pair() {
        // Different tx hashes: this is not a shift, so the classic pair is
        // correct.
        let kinds = compare(
            &[chain_settlement(10, 1, 1)],
            &[],
            &[db_settlement(20, 2, 1)],
            &[],
        );
        assert_eq!(kinds, vec!["settlement_not_on_chain", "settlement_missing"]);
    }
}
