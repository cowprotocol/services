//! Locating orphaned DB trades on-chain, shared by the check and repair
//! subcommands.

use {
    crate::{
        chain::{ChainTrade, SettlementSource, SettlementTx, fetch_settlements},
        db::{DbTrade, ORPHANED_TRADES_QUERY, table_stats},
    },
    alloy_primitives::{Address, B256, U256, hex},
    alloy_provider::Provider,
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    number::conversions::u256_to_big_decimal,
    sqlx::PgConnection,
    std::collections::BTreeMap,
};

/// An on-chain Trade event with the same order uid as a DB trade. `diffs`
/// lists the fields whose values differ from the DB data; an exact match has
/// no diffs.
pub struct Candidate {
    pub block: u64,
    pub tx_hash: B256,
    pub log_index: u64,
    pub diffs: Vec<String>,
}

/// Compares a DB trade against an on-chain Trade event. Returns None if the
/// order uids differ, otherwise the list of mismatching fields.
fn compare(db: &DbTrade, event: &ChainTrade) -> Option<Vec<String>> {
    if db.order_uid != event.order_uid {
        return None;
    }
    let mut diffs = Vec::new();
    {
        let mut check_amount = |name: &str, chain: &U256, db_value: &BigDecimal| {
            if u256_to_big_decimal(chain) != *db_value {
                diffs.push(format!("{name} chain {chain} != db {db_value}"));
            }
        };
        check_amount("sell_amount", &event.sell_amount, &db.sell_amount);
        check_amount("buy_amount", &event.buy_amount, &db.buy_amount);
        check_amount("fee_amount", &event.fee_amount, &db.fee_amount);
    }
    {
        let mut check_address = |name: &str, chain: &Address, db_value: &Option<Vec<u8>>| {
            if let Some(db_value) = db_value
                && db_value.as_slice() != chain.as_slice()
            {
                diffs.push(format!(
                    "{name} chain {chain} != db {}",
                    hex::encode_prefixed(db_value)
                ));
            }
        };
        check_address("sell_token", &event.sell_token, &db.sell_token);
        check_address("buy_token", &event.buy_token, &db.buy_token);
        check_address("owner", &event.owner, &db.owner);
    }
    Some(diffs)
}

fn candidates(db_trade: &DbTrade, txs: &[SettlementTx]) -> Vec<Candidate> {
    txs.iter()
        .flat_map(|tx| {
            tx.trades.iter().filter_map(|event| {
                Some(Candidate {
                    block: tx.block,
                    tx_hash: tx.tx_hash,
                    log_index: event.log_index,
                    diffs: compare(db_trade, event)?,
                })
            })
        })
        .collect()
}

/// The outcome of locating one orphaned DB trade on-chain.
pub struct TradeReport {
    /// The block the DB recorded the trade at.
    pub block: u64,
    pub trade: DbTrade,
    /// Exact matches: same uid, amounts and (when known) tokens/owner.
    pub matches: Vec<Candidate>,
    /// Same uid but different data.
    pub near_misses: Vec<Candidate>,
}

impl TradeReport {
    pub fn status(&self) -> &'static str {
        match self.matches.len() {
            0 => "not_found",
            1 => "located",
            _ => "ambiguous",
        }
    }

    pub fn order_note(&self) -> &'static str {
        match self.trade.sell_token {
            Some(_) => "",
            None => "order not in orders/jit_orders; matched on uid and amounts only",
        }
    }
}

/// Finds all orphaned DB trades and locates them on-chain. Aborts before any
/// RPC call if more than `max_orphan_blocks` distinct blocks are affected,
/// which usually means an empty or wrong database rather than genuine damage.
pub async fn locate_orphans(
    provider: &impl Provider,
    sources: &[SettlementSource],
    db: &mut PgConnection,
    window: u64,
    max_orphan_blocks: u64,
) -> Result<Vec<TradeReport>> {
    let stats = table_stats(db).await?;
    tracing::info!(
        trades = stats.trades,
        settlements = stats.settlements,
        min_trade_block = ?stats.min_trade_block,
        max_trade_block = ?stats.max_trade_block,
        min_settlement_block = ?stats.min_settlement_block,
        max_settlement_block = ?stats.max_settlement_block,
        "database table stats"
    );

    let trades: Vec<DbTrade> = sqlx::query_as(ORPHANED_TRADES_QUERY)
        .fetch_all(&mut *db)
        .await
        .context("could not query trades without settlements")?;

    let mut by_block: BTreeMap<u64, Vec<DbTrade>> = BTreeMap::new();
    for trade in trades {
        let block = u64::try_from(trade.block_number).context("negative block number")?;
        by_block.entry(block).or_default().push(trade);
    }
    tracing::info!(
        trades = by_block.values().map(Vec::len).sum::<usize>(),
        blocks = by_block.len(),
        "DB trades without settlement event"
    );
    anyhow::ensure!(
        by_block.len() as u64 <= max_orphan_blocks,
        "{} orphaned blocks exceed the --max-orphan-blocks limit of {max_orphan_blocks}; this \
         usually means an empty or wrong database (check the table stats above) rather than \
         genuine damage. Raise the limit to proceed.",
        by_block.len()
    );

    let mut reports = Vec::new();
    for (block, group) in by_block {
        tracing::info!(block, orphaned_trades = group.len(), "checking block");
        let target = fetch_settlements(provider, sources, block, block).await?;
        // Fetched lazily, only when a trade is not found in the target block,
        // and reused by all trades of the block.
        let mut neighborhood: Option<Vec<SettlementTx>> = None;
        for db_trade in group {
            let mut cands = candidates(&db_trade, &target);
            if !cands.iter().any(|c| c.diffs.is_empty()) && window > 0 {
                let txs = match &neighborhood {
                    Some(txs) => txs,
                    None => {
                        let from = block.saturating_sub(window);
                        let to = block.saturating_add(window);
                        tracing::info!(from, to, "searching neighbor blocks");
                        neighborhood.insert(fetch_settlements(provider, sources, from, to).await?)
                    }
                };
                cands = candidates(&db_trade, txs);
            }
            let (matches, near_misses) = cands.into_iter().partition(|c| c.diffs.is_empty());
            reports.push(TradeReport {
                block,
                trade: db_trade,
                matches,
                near_misses,
            });
        }
    }
    Ok(reports)
}
