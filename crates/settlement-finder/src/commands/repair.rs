//! The `repair` subcommand: re-index the block ranges around orphaned trades
//! from canonical chain data.
//!
//! Re-indexing deletes every event row of a block range and re-inserts what
//! the chain actually has there, so it is only safe when nothing correct is
//! lost. Before deleting a range the tool therefore accounts for every DB row
//! it would remove (see `account_range`) and refuses the range unless each row
//! is either re-created from the canonical logs or provably preserved
//! elsewhere. Each range is its own transaction; `--apply` commits, otherwise
//! it is rolled back after a full rehearsal.

use {
    crate::{
        chain::{
            CanonicalEvents,
            SettlementSource,
            decode_canonical,
            fetch_logs,
            format_sources,
            validate_canonical_hashes,
        },
        db::{db_settlements_by_tx, trades_have_tx_hash},
        orphans::{TradeReport, locate_orphans},
    },
    alloy_primitives::{B256, hex},
    alloy_provider::Provider,
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    number::conversions::u256_to_big_decimal,
    sqlx::{Connection, PgConnection},
    std::collections::{BTreeMap, BTreeSet},
};

// The re-index statements. The DELETEs clear a whole block range; the INSERTs
// mirror the columns the event indexer writes (crates/database/src/events.rs)
// so they can be audited against it. Unlike the indexer they omit `ON CONFLICT
// DO NOTHING`: the range was just deleted, so any conflict is a bug we want to
// surface (the surrounding transaction rolls back on error).
const DELETE_TRADES: &str = "DELETE FROM trades WHERE block_number BETWEEN $1 AND $2;";
const DELETE_SETTLEMENTS: &str = "DELETE FROM settlements WHERE block_number BETWEEN $1 AND $2;";
const DELETE_INVALIDATIONS: &str =
    "DELETE FROM invalidations WHERE block_number BETWEEN $1 AND $2;";
const DELETE_PRESIGNATURES: &str =
    "DELETE FROM presignature_events WHERE block_number BETWEEN $1 AND $2;";

const INSERT_TRADE: &str = "INSERT INTO trades (block_number, log_index, order_uid, sell_amount, \
                            buy_amount, fee_amount, tx_hash) VALUES ($1, $2, $3, $4, $5, $6, $7);";
const INSERT_TRADE_WITHOUT_TX_HASH: &str = "INSERT INTO trades (block_number, log_index, \
                                            order_uid, sell_amount, buy_amount, fee_amount) \
                                            VALUES ($1, $2, $3, $4, $5, $6);";
const INSERT_SETTLEMENT: &str =
    "INSERT INTO settlements (block_number, log_index, solver, tx_hash) VALUES ($1, $2, $3, $4);";
const INSERT_INVALIDATION: &str =
    "INSERT INTO invalidations (block_number, log_index, order_uid) VALUES ($1, $2, $3);";
const INSERT_PRESIGNATURE: &str = "INSERT INTO presignature_events (block_number, log_index, \
                                   owner, order_uid, signed) VALUES ($1, $2, $3, $4, $5);";
const RESTORE_AUCTION: &str = "UPDATE settlements SET auction_id = $1, solution_uid = $2 WHERE \
                               block_number = $3 AND log_index = $4;";

/// Widest range the tool will re-index for a single orphan; a wider hull almost
/// certainly means a wrong match or database rather than a reorg.
const MAX_RANGE_WIDTH: u64 = 1000;

/// A settlements row whose auction data must survive the re-index.
#[derive(sqlx::FromRow)]
struct SavedAuction {
    tx_hash: Vec<u8>,
    auction_id: Option<i64>,
    solution_uid: Option<i64>,
}

/// A settlements row the DELETE would remove, for accounting.
#[derive(sqlx::FromRow)]
struct DbSettlementRow {
    block_number: i64,
    log_index: i64,
    tx_hash: Vec<u8>,
}

/// A trades row the DELETE would remove, for accounting.
#[derive(sqlx::FromRow)]
struct DbTradeRow {
    block_number: i64,
    log_index: i64,
    order_uid: Vec<u8>,
    sell_amount: BigDecimal,
    buy_amount: BigDecimal,
    fee_amount: BigDecimal,
}

/// The auction data to reattach to one re-inserted settlement.
struct AuctionRestore {
    block: i64,
    log_index: i64,
    auction_id: Option<i64>,
    solution_uid: Option<i64>,
}

/// Pairs saved auction data to the re-inserted canonical settlements by tx
/// hash, in (block, log_index) rank order for the rare case of multiple
/// settlements per transaction.
fn plan_auction_restores(
    canonical: &CanonicalEvents,
    saved: &[SavedAuction],
) -> Vec<AuctionRestore> {
    // Rank pairing assumes the DB rows' (block, log_index) order matches the
    // canonical order of the tx's settlements — a fork can have swapped it, so
    // flag txs where the pairing distributes non-identical auction data.
    type AuctionData = (Option<i64>, Option<i64>);
    let mut saved_by_tx: BTreeMap<&[u8], Vec<AuctionData>> = BTreeMap::new();
    for row in saved {
        saved_by_tx
            .entry(row.tx_hash.as_slice())
            .or_default()
            .push((row.auction_id, row.solution_uid));
    }
    for (tx, data) in &saved_by_tx {
        if data.len() > 1 && data.windows(2).any(|pair| pair[0] != pair[1]) {
            tracing::warn!(
                tx = %hex::encode_prefixed(tx),
                "multiple settlements of this tx carry different auction data; rank-order \
                 pairing may swap them, verify auction_id/solution_uid after the repair"
            );
        }
    }

    let mut canonical_by_tx: BTreeMap<&[u8], Vec<(i64, i64)>> = BTreeMap::new();
    for settlement in &canonical.settlements {
        canonical_by_tx
            .entry(settlement.tx_hash.as_slice())
            .or_default()
            .push((
                settlement.block.cast_signed(),
                settlement.log_index.cast_signed(),
            ));
    }

    let mut used: BTreeMap<&[u8], usize> = BTreeMap::new();
    let mut restores = Vec::new();
    for row in saved {
        let rank = used.entry(row.tx_hash.as_slice()).or_default();
        match canonical_by_tx
            .get(row.tx_hash.as_slice())
            .and_then(|settlements| settlements.get(*rank))
        {
            Some(&(block, log_index)) => restores.push(AuctionRestore {
                block,
                log_index,
                auction_id: row.auction_id,
                solution_uid: row.solution_uid,
            }),
            // More saved rows for this tx than canonical settlements: the tx
            // was indexed at several (fork) coordinates but the chain has fewer
            // occurrences. The earlier ranks already captured the auction data;
            // this extra copy is a duplicate, not a loss, unless it disagrees.
            None => tracing::warn!(
                tx = %hex::encode_prefixed(&row.tx_hash),
                auction_id = ?row.auction_id,
                solution_uid = ?row.solution_uid,
                "not restoring auction data of a duplicate settlements row (more DB copies of \
                 this tx than on-chain occurrences)"
            ),
        }
        *rank += 1;
    }
    restores
}

/// Whether an interval set covers a block.
fn covered(intervals: &[(u64, u64)], block: u64) -> bool {
    intervals
        .iter()
        .any(|&(from, to)| (from..=to).contains(&block))
}

/// Checks that deleting `[from, to]` and re-inserting `canonical` loses no
/// correct data. Returns the blocking reasons (empty means safe); emits
/// warnings for deletions that are the intended cleanup. Performs one receipt
/// lookup per DB settlement whose tx is absent from the fetched logs.
#[allow(clippy::too_many_arguments)]
async fn account_range(
    provider: &impl Provider,
    db: &mut PgConnection,
    from: u64,
    to: u64,
    canonical: &CanonicalEvents,
    intervals: &[(u64, u64)],
    located_in_range: &[(u64, B256, u64)],
    trades_have_tx_hash: bool,
) -> Result<Vec<String>> {
    let mut problems = Vec::new();

    // Every located match must appear in the fetched logs; otherwise the RPC
    // returned a partial or empty result and the DELETE would wipe more than
    // it restores.
    for &(block, tx_hash, log_index) in located_in_range {
        let present = canonical
            .trades
            .iter()
            .any(|t| t.block == block && t.log_index == log_index && t.tx_hash == tx_hash);
        if !present {
            problems.push(format!(
                "located trade tx {} at {block}/{log_index} is absent from the fetched logs (RPC \
                 returned a partial/empty result)",
                hex::encode_prefixed(tx_hash.as_slice())
            ));
        }
    }

    // Settlements: the anchor. Each DB row must be re-created here, be fork
    // garbage the chain no longer has (intended deletion), or live at a block
    // some range covers.
    let canonical_settlement_txs: BTreeSet<&[u8]> = canonical
        .settlements
        .iter()
        .map(|s| s.tx_hash.as_slice())
        .collect();
    let db_settlements: Vec<DbSettlementRow> = sqlx::query_as(
        "SELECT block_number, log_index, tx_hash FROM settlements WHERE block_number BETWEEN $1 \
         AND $2",
    )
    .bind(from.cast_signed())
    .bind(to.cast_signed())
    .fetch_all(&mut *db)
    .await?;
    for row in &db_settlements {
        if canonical_settlement_txs.contains(row.tx_hash.as_slice()) {
            continue;
        }
        let tx = hex::encode_prefixed(&row.tx_hash);
        let hash = B256::try_from(row.tx_hash.as_slice()).with_context(|| {
            format!(
                "settlements row {}/{} has a malformed tx_hash {tx}",
                row.block_number, row.log_index
            )
        })?;
        match provider
            .get_transaction_receipt(hash)
            .await
            .context("could not fetch settlement tx receipt")?
        {
            None => {
                // A missing receipt is how a fork artifact looks, but also how
                // a pruned or lagging node answers; only treat it as fork
                // garbage when the node consistently knows nothing about the
                // transaction either.
                let tx_known = provider
                    .get_transaction_by_hash(hash)
                    .await
                    .context("could not fetch settlement tx")?
                    .is_some();
                if tx_known {
                    problems.push(format!(
                        "settlements tx {tx} has no receipt although the node knows the \
                         transaction; refusing to treat it as a fork artifact (inconsistent node)"
                    ));
                } else {
                    tracing::warn!(
                        block = row.block_number,
                        log_index = row.log_index,
                        %tx,
                        "deleting a settlements row whose transaction is not on the canonical \
                         chain (fork artifact)"
                    );
                }
            }
            Some(receipt) => match receipt.block_number {
                None => problems.push(format!(
                    "settlements tx {tx} has a receipt without a block number (still pending?); \
                     refusing to delete it"
                )),
                Some(canonical_block) if (from..=to).contains(&canonical_block) => {
                    problems.push(format!(
                        "settlements tx {tx} is canonically at block {canonical_block} inside the \
                         range but missing from the fetched logs (RPC gap)"
                    ))
                }
                Some(canonical_block) if covered(intervals, canonical_block) => tracing::info!(
                    %tx,
                    canonical_block,
                    "settlements row will be re-created by the range covering its canonical block"
                ),
                Some(canonical_block) => problems.push(format!(
                    "settlements tx {tx} is canonically at block {canonical_block}, outside every \
                     repair range; widen --window so the range includes it"
                )),
            },
        }
    }

    // Trades: each DB row must be re-created here or already have an identical
    // copy outside every repair interval (its canonical home, never disturbed;
    // a copy inside another interval would be deleted by that interval's own
    // repair, so it cannot vouch for this one).
    let db_trades: Vec<DbTradeRow> = sqlx::query_as(
        "SELECT block_number, log_index, order_uid, sell_amount, buy_amount, fee_amount FROM \
         trades WHERE block_number BETWEEN $1 AND $2",
    )
    .bind(from.cast_signed())
    .bind(to.cast_signed())
    .fetch_all(&mut *db)
    .await?;

    // Count identical fills on each side: a plain any() lets one canonical
    // trade vouch for several identical DB rows. A DB surplus is usually the
    // fork dedup this command exists for, but it deserves a visible trace.
    type FillKey<'a> = (&'a [u8], BigDecimal, BigDecimal, BigDecimal);
    let mut canonical_fills: BTreeMap<FillKey, usize> = BTreeMap::new();
    for t in &canonical.trades {
        *canonical_fills
            .entry((
                t.order_uid.as_slice(),
                u256_to_big_decimal(&t.sell_amount),
                u256_to_big_decimal(&t.buy_amount),
                u256_to_big_decimal(&t.fee_amount),
            ))
            .or_default() += 1;
    }
    let mut db_fills: BTreeMap<FillKey, usize> = BTreeMap::new();
    for row in &db_trades {
        *db_fills
            .entry((
                row.order_uid.as_slice(),
                row.sell_amount.clone(),
                row.buy_amount.clone(),
                row.fee_amount.clone(),
            ))
            .or_default() += 1;
    }
    for (key, &db_count) in &db_fills {
        let canonical_count = canonical_fills.get(key).copied().unwrap_or(0);
        if db_count > canonical_count && canonical_count > 0 {
            tracing::warn!(
                order = %hex::encode_prefixed(key.0),
                db_count,
                canonical_count,
                "deleting surplus identical fills of this order (fork copies not re-created)"
            );
        }
    }

    for row in &db_trades {
        let reproduced = canonical_fills.contains_key(&(
            row.order_uid.as_slice(),
            row.sell_amount.clone(),
            row.buy_amount.clone(),
            row.fee_amount.clone(),
        ));
        if reproduced {
            continue;
        }
        let copies: Vec<(i64,)> = sqlx::query_as(
            "SELECT block_number FROM trades WHERE order_uid = $1 AND sell_amount = $2 AND \
             buy_amount = $3 AND fee_amount = $4",
        )
        .bind(row.order_uid.as_slice())
        .bind(&row.sell_amount)
        .bind(&row.buy_amount)
        .bind(&row.fee_amount)
        .fetch_all(&mut *db)
        .await?;
        let copy_preserved = copies
            .iter()
            .any(|&(block,)| u64::try_from(block).is_ok_and(|block| !covered(intervals, block)));
        if !copy_preserved {
            problems.push(format!(
                "trades row {}/{} (order {}) is not reproduced by the fetched logs and has no \
                 copy outside the repair ranges; deleting it would lose the only record of this \
                 fill",
                row.block_number,
                row.log_index,
                hex::encode_prefixed(&row.order_uid)
            ));
        }
    }

    // Invalidations and presignatures are re-created from the same logs;
    // report (but do not block on) fork copies that will not come back, since
    // they carry no value that another table depends on.
    for (table, canonical_count) in [
        (
            "invalidations",
            i64::try_from(canonical.invalidations.len()).unwrap_or(i64::MAX),
        ),
        (
            "presignature_events",
            i64::try_from(canonical.presignatures.len()).unwrap_or(i64::MAX),
        ),
    ] {
        let (db_count,): (i64,) = sqlx::query_as(&format!(
            "SELECT count(*) FROM {table} WHERE block_number BETWEEN $1 AND $2"
        ))
        .bind(from.cast_signed())
        .bind(to.cast_signed())
        .fetch_one(&mut *db)
        .await?;
        if db_count > canonical_count {
            tracing::warn!(
                table,
                db_count,
                canonical_count,
                "deleting more rows than the chain has in this range (fork copies not re-created)"
            );
        }
    }

    let _ = trades_have_tx_hash;
    Ok(problems)
}

/// Wipes the event tables in `from..=to` and re-inserts the canonical events,
/// then reattaches the saved auction data. Runs against a transaction the
/// caller commits or rolls back.
async fn reindex_range(
    conn: &mut PgConnection,
    from: u64,
    to: u64,
    canonical: &CanonicalEvents,
    restores: &[AuctionRestore],
    trades_have_tx_hash: bool,
) -> Result<()> {
    let (from, to) = (from.cast_signed(), to.cast_signed());
    for query in [
        DELETE_TRADES,
        DELETE_SETTLEMENTS,
        DELETE_INVALIDATIONS,
        DELETE_PRESIGNATURES,
    ] {
        sqlx::query(query)
            .bind(from)
            .bind(to)
            .execute(&mut *conn)
            .await?;
    }

    for trade in &canonical.trades {
        let sell = u256_to_big_decimal(&trade.sell_amount);
        let buy = u256_to_big_decimal(&trade.buy_amount);
        let fee = u256_to_big_decimal(&trade.fee_amount);
        if trades_have_tx_hash {
            sqlx::query(INSERT_TRADE)
                .bind(trade.block.cast_signed())
                .bind(trade.log_index.cast_signed())
                .bind(trade.order_uid.as_slice())
                .bind(&sell)
                .bind(&buy)
                .bind(&fee)
                .bind(trade.tx_hash.as_slice())
                .execute(&mut *conn)
                .await?;
        } else {
            sqlx::query(INSERT_TRADE_WITHOUT_TX_HASH)
                .bind(trade.block.cast_signed())
                .bind(trade.log_index.cast_signed())
                .bind(trade.order_uid.as_slice())
                .bind(&sell)
                .bind(&buy)
                .bind(&fee)
                .execute(&mut *conn)
                .await?;
        }
    }

    for settlement in &canonical.settlements {
        sqlx::query(INSERT_SETTLEMENT)
            .bind(settlement.block.cast_signed())
            .bind(settlement.log_index.cast_signed())
            .bind(settlement.solver.as_slice())
            .bind(settlement.tx_hash.as_slice())
            .execute(&mut *conn)
            .await?;
    }

    for invalidation in &canonical.invalidations {
        sqlx::query(INSERT_INVALIDATION)
            .bind(invalidation.block.cast_signed())
            .bind(invalidation.log_index.cast_signed())
            .bind(invalidation.order_uid.as_slice())
            .execute(&mut *conn)
            .await?;
    }

    for presignature in &canonical.presignatures {
        sqlx::query(INSERT_PRESIGNATURE)
            .bind(presignature.block.cast_signed())
            .bind(presignature.log_index.cast_signed())
            .bind(presignature.owner.as_slice())
            .bind(presignature.order_uid.as_slice())
            .bind(presignature.signed)
            .execute(&mut *conn)
            .await?;
    }

    for restore in restores {
        let result = sqlx::query(RESTORE_AUCTION)
            .bind(restore.auction_id)
            .bind(restore.solution_uid)
            .bind(restore.block)
            .bind(restore.log_index)
            .execute(&mut *conn)
            .await?;
        anyhow::ensure!(
            result.rows_affected() == 1,
            "auction restore updated {} settlements rows at {}/{}, expected exactly 1",
            result.rows_affected(),
            restore.block,
            restore.log_index
        );
    }

    Ok(())
}

/// Merges overlapping or adjacent intervals.
fn merge_intervals(mut intervals: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    intervals.sort();
    let mut merged: Vec<(u64, u64)> = Vec::new();
    for (from, to) in intervals {
        match merged.last_mut() {
            Some((_, last_to)) if from <= last_to.saturating_add(1) => {
                *last_to = (*last_to).max(to);
            }
            _ => merged.push((from, to)),
        }
    }
    merged
}

/// A trade the tool will not repair, with the reason to log.
struct SkippedTrade {
    block: u64,
    log_index: i64,
    reason: &'static str,
}

/// Splits located reports into repairable ones and those skipped because the
/// order has more than one identically-sized fill in the DB (the single
/// located match could then be a different fill; re-indexing risks losing the
/// real one).
async fn split_by_fill_uniqueness(
    db: &mut PgConnection,
    located: Vec<TradeReport>,
) -> Result<(Vec<TradeReport>, Vec<SkippedTrade>)> {
    let mut repairable = Vec::new();
    let mut skipped = Vec::new();
    for report in located {
        let (fills,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM trades WHERE order_uid = $1 AND sell_amount = $2 AND buy_amount \
             = $3 AND fee_amount = $4",
        )
        .bind(report.trade.order_uid.as_slice())
        .bind(&report.trade.sell_amount)
        .bind(&report.trade.buy_amount)
        .bind(&report.trade.fee_amount)
        .fetch_one(&mut *db)
        .await?;
        if fills > 1 {
            skipped.push(SkippedTrade {
                block: report.block,
                log_index: report.trade.log_index,
                reason: "order has multiple identically-sized fills; the located match cannot be \
                         disambiguated",
            });
        } else {
            repairable.push(report);
        }
    }
    Ok((repairable, skipped))
}

/// Outcome of processing one interval.
enum RangeOutcome {
    Committed,
    Rehearsed,
    Aborted,
}

#[allow(clippy::too_many_arguments)]
async fn process_range(
    provider: &impl Provider,
    sources: &[SettlementSource],
    db: &mut PgConnection,
    from: u64,
    to: u64,
    intervals: &[(u64, u64)],
    located: &[(u64, B256, u64)],
    skipped_blocks: &BTreeSet<u64>,
    finalized_head: u64,
    trades_have_tx_hash: bool,
    apply: bool,
) -> Result<RangeOutcome> {
    if to > finalized_head {
        tracing::warn!(
            from,
            to,
            finalized_head,
            "skipping range: it reaches unfinalized blocks, which may still reorg; lower \
             --finality or wait"
        );
        return Ok(RangeOutcome::Aborted);
    }
    if let Some(&block) = skipped_blocks.iter().find(|&&b| (from..=to).contains(&b)) {
        tracing::warn!(
            from,
            to,
            block,
            "skipping range: it contains a trade that was itself skipped, which the DELETE would \
             remove without re-creating"
        );
        return Ok(RangeOutcome::Aborted);
    }

    // These logs get written into the DB, so they must provably belong to the
    // canonical chain: a node whose log index serves another block's logs
    // under the queried number (observed on public Gnosis endpoints) would
    // otherwise poison the very tables this command repairs.
    let logs = fetch_logs(provider, sources, from, to).await?;
    validate_canonical_hashes(provider, &logs).await?;
    let canonical = decode_canonical(&logs);
    let located_in_range: Vec<(u64, B256, u64)> = located
        .iter()
        .copied()
        .filter(|&(block, _, _)| (from..=to).contains(&block))
        .collect();

    let problems = account_range(
        provider,
        db,
        from,
        to,
        &canonical,
        intervals,
        &located_in_range,
        trades_have_tx_hash,
    )
    .await?;
    if !problems.is_empty() {
        for problem in &problems {
            tracing::warn!(from, to, problem, "range failed accounting");
        }
        return Ok(RangeOutcome::Aborted);
    }

    let saved: Vec<SavedAuction> = sqlx::query_as(
        "SELECT tx_hash, auction_id, solution_uid FROM settlements WHERE block_number BETWEEN $1 \
         AND $2 AND (auction_id IS NOT NULL OR solution_uid IS NOT NULL) ORDER BY block_number, \
         log_index",
    )
    .bind(from.cast_signed())
    .bind(to.cast_signed())
    .fetch_all(&mut *db)
    .await?;
    let (jit_rows,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM jit_orders WHERE block_number BETWEEN $1 AND $2")
            .bind(from.cast_signed())
            .bind(to.cast_signed())
            .fetch_one(&mut *db)
            .await?;
    if jit_rows > 0 {
        tracing::info!(from, to, jit_rows, "leaving jit_orders rows untouched");
    }

    let restores = plan_auction_restores(&canonical, &saved);
    let mut tx = db.begin().await?;
    reindex_range(
        &mut tx,
        from,
        to,
        &canonical,
        &restores,
        trades_have_tx_hash,
    )
    .await?;

    if apply {
        tx.commit().await?;
        tracing::info!(
            from,
            to,
            trades = canonical.trades.len(),
            settlements = canonical.settlements.len(),
            "re-indexed range"
        );
        Ok(RangeOutcome::Committed)
    } else {
        tx.rollback().await?;
        tracing::info!(
            from,
            to,
            trades = canonical.trades.len(),
            settlements = canonical.settlements.len(),
            "dry run: re-indexed range and rolled back"
        );
        Ok(RangeOutcome::Rehearsed)
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn repair_cmd(
    provider: &impl Provider,
    sources: &[SettlementSource],
    chain_id: u64,
    db_url: &str,
    window: u64,
    max_orphan_blocks: u64,
    finality: u64,
    apply: bool,
) -> Result<()> {
    let mut db = PgConnection::connect(db_url)
        .await
        .context("could not connect to database")?;
    let reports = locate_orphans(provider, sources, &mut db, window, max_orphan_blocks).await?;
    if reports.is_empty() {
        tracing::info!("every DB trade resolves to a settlement event, nothing to repair");
        return Ok(());
    }

    let (located, not_located): (Vec<_>, Vec<_>) =
        reports.into_iter().partition(|r| r.matches.len() == 1);
    let mut skipped: Vec<SkippedTrade> = not_located
        .into_iter()
        .map(|r| SkippedTrade {
            block: r.block,
            log_index: r.trade.log_index,
            reason: match r.status() {
                "ambiguous" => "several equally good matches on-chain",
                _ => "no matching trade found on-chain",
            },
        })
        .collect();
    let (repairable, fill_skips) = split_by_fill_uniqueness(&mut db, located).await?;
    skipped.extend(fill_skips);

    // The range of a trade spans the block the DB recorded, the located block,
    // and any settlements rows of the same tx hash (the settlement itself may
    // have been indexed from an orphaned fork).
    let located: Vec<(u64, B256, u64)> = repairable
        .iter()
        .map(|r| {
            let c = &r.matches[0];
            (c.block, c.tx_hash, c.log_index)
        })
        .collect();
    let mut intervals = Vec::new();
    for report in &repairable {
        let candidate = &report.matches[0];
        let mut from = report.block.min(candidate.block);
        let mut to = report.block.max(candidate.block);
        for row in db_settlements_by_tx(&mut db, candidate.tx_hash.as_slice()).await? {
            let block = u64::try_from(row.block_number).context("negative block number")?;
            from = from.min(block);
            to = to.max(block);
        }
        if to - from > MAX_RANGE_WIDTH {
            tracing::warn!(
                block = report.block,
                log_index = report.trade.log_index,
                from,
                to,
                "skipping trade: repair range is implausibly wide, investigate manually"
            );
            skipped.push(SkippedTrade {
                block: report.block,
                log_index: report.trade.log_index,
                reason: "repair range implausibly wide",
            });
            continue;
        }
        intervals.push((from, to));
    }
    let intervals = merge_intervals(intervals);

    for skip in &skipped {
        tracing::warn!(
            block = skip.block,
            log_index = skip.log_index,
            skip.reason,
            "skipping trade"
        );
    }
    if intervals.is_empty() {
        tracing::error!(skipped = skipped.len(), "no repairable trades");
        std::process::exit(1);
    }

    let skipped_blocks: BTreeSet<u64> = skipped.iter().map(|s| s.block).collect();
    let head = provider
        .get_block_number()
        .await
        .context("could not fetch chain head")?;
    let finalized_head = head.saturating_sub(finality);
    let trades_have_tx_hash = trades_have_tx_hash(&mut db).await?;

    tracing::info!(
        chain_id,
        contract = %format_sources(sources),
        trades = repairable.len(),
        ranges = intervals.len(),
        skipped = skipped.len(),
        finalized_head,
        apply,
        "repairing orphaned trades"
    );

    let (mut committed, mut rehearsed, mut aborted) = (0usize, 0usize, 0usize);
    for &(from, to) in &intervals {
        let outcome = process_range(
            provider,
            sources,
            &mut db,
            from,
            to,
            &intervals,
            &located,
            &skipped_blocks,
            finalized_head,
            trades_have_tx_hash,
            apply,
        )
        .await;
        match outcome {
            Ok(RangeOutcome::Committed) => committed += 1,
            Ok(RangeOutcome::Rehearsed) => rehearsed += 1,
            Ok(RangeOutcome::Aborted) => aborted += 1,
            Err(err) => {
                aborted += 1;
                tracing::error!(from, to, ?err, "range failed; continuing with the rest");
            }
        }
    }

    if apply {
        let (remaining,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM trades t WHERE NOT EXISTS (SELECT 1 FROM settlements s WHERE \
             s.block_number = t.block_number AND s.log_index > t.log_index)",
        )
        .fetch_one(&mut db)
        .await?;
        tracing::info!(
            committed,
            aborted,
            skipped = skipped.len(),
            remaining_orphans = remaining,
            "repair complete"
        );
    } else {
        tracing::info!(
            rehearsed,
            aborted,
            skipped = skipped.len(),
            "dry run complete; re-run with --apply to commit the changes"
        );
    }

    // Exit codes for orchestration: 0 clean, 2 partial (committed some but not
    // all), 1 nothing usefully done / dry-run found issues.
    let incomplete = aborted > 0 || !skipped.is_empty();
    let code = match (apply, committed, incomplete) {
        (_, _, false) => 0,
        (true, c, true) if c > 0 => 2,
        _ => 1,
    };
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}
