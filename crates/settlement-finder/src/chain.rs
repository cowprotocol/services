//! Fetching and decoding settlement contract logs from the chain.

use {
    alloy_primitives::{Address, B256, U256},
    alloy_provider::Provider,
    alloy_rpc_types::{Filter, Log},
    alloy_sol_types::SolEventInterface,
    anyhow::{Context, Result},
    contracts::GPv2Settlement::GPv2Settlement::GPv2SettlementEvents,
    std::{fmt, time::Duration},
};

/// getLogs is idempotent and read-only, so transient failures (connection
/// resets, body-decode errors, provider rate limits like Alchemy's 429) are
/// safe to retry. We back off exponentially and only surface an error once the
/// attempts are exhausted; a genuinely permanent error (e.g. a bad block range)
/// still fails, just after a bounded delay.
const MAX_LOG_FETCH_ATTEMPTS: u32 = 8;
const RETRY_BASE_DELAY: Duration = Duration::from_millis(200);
const RETRY_MAX_DELAY: Duration = Duration::from_secs(10);

/// Whether an RPC error means "the block range or result set is too large" —
/// a deterministic error (unlike a transient network blip) that retrying the
/// same range cannot fix, but which we can recover from by splitting the range.
/// Covers reth ("query exceeds max results", "query exceeds max block range"),
/// Alchemy/Infura ("query returned more than N results", "response size
/// exceeded", "up to a N block range") and similar phrasings.
fn is_range_too_large(err: &impl fmt::Display) -> bool {
    let message = err.to_string().to_lowercase();
    [
        "max results",
        "max block range",
        "more than",
        "response size",
        "block range",
        "range is too large",
        "too large",
        "limit exceeded",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

/// A settlement contract to read events from, optionally restricted to the
/// block window during which it was the active deployment. Events emitted by
/// the address outside `[from_block, to_block]` are ignored, so a
/// contract-migration boundary (mainnet used
/// 0x3328f5f2cEcAF00a2443082B657CedEAf70bfAEf before switching to
/// 0x9008D19f58AAbD9eD0D60971565AA8510560ab41) can be expressed exactly.
#[derive(Clone)]
pub struct SettlementSource {
    pub address: Address,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
}

impl SettlementSource {
    /// Whether the address was the active deployment at `block`.
    fn active_at(&self, block: u64) -> bool {
        self.from_block.is_none_or(|from| block >= from)
            && self.to_block.is_none_or(|to| block <= to)
    }

    /// Whether the active window overlaps the `[from, to]` query range.
    fn overlaps(&self, from: u64, to: u64) -> bool {
        self.from_block.unwrap_or(0) <= to && from <= self.to_block.unwrap_or(u64::MAX)
    }
}

impl fmt::Display for SettlementSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.address)?;
        match (self.from_block, self.to_block) {
            (None, None) => Ok(()),
            (from, to) => {
                let fmt_bound = |b: Option<u64>| b.map(|b| b.to_string()).unwrap_or_default();
                write!(f, ":{}-{}", fmt_bound(from), fmt_bound(to))
            }
        }
    }
}

/// Parses a `--settlement` value: an address, optionally suffixed with a block
/// window `:FROM-TO`. Either side of the range may be empty for an open end,
/// e.g. `0x9008…:12500000-` (from 12.5M on) or `0x3328…:-12500000` (up to
/// 12.5M). A bare address is active for all blocks.
pub fn parse_settlement_source(value: &str) -> Result<SettlementSource, String> {
    let (address, range) = match value.split_once(':') {
        Some((address, range)) => (address, Some(range)),
        None => (value, None),
    };
    let address = address
        .trim()
        .parse::<Address>()
        .map_err(|err| format!("invalid settlement address '{address}': {err}"))?;

    let parse_bound = |bound: &str, which: &str| -> Result<Option<u64>, String> {
        let bound = bound.trim();
        if bound.is_empty() {
            Ok(None)
        } else {
            bound
                .parse::<u64>()
                .map(Some)
                .map_err(|err| format!("invalid {which} block '{bound}': {err}"))
        }
    };
    let (from_block, to_block) = match range {
        None => (None, None),
        Some(range) => {
            let (from, to) = range.split_once('-').ok_or_else(|| {
                format!(
                    "settlement block window '{range}' must be FROM-TO; use an empty side for an \
                     open end, e.g. 12500000- or -12500000"
                )
            })?;
            (parse_bound(from, "from")?, parse_bound(to, "to")?)
        }
    };
    if let (Some(from), Some(to)) = (from_block, to_block)
        && from > to
    {
        return Err(format!(
            "settlement block window is empty: from {from} is above to {to}"
        ));
    }
    Ok(SettlementSource {
        address,
        from_block,
        to_block,
    })
}

/// Joins settlement sources for one-line display.
pub fn format_sources(sources: &[SettlementSource]) -> String {
    sources
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// A Trade event as found on-chain.
pub struct ChainTrade {
    pub log_index: u64,
    pub owner: Address,
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
    pub order_uid: Vec<u8>,
}

/// All events a settlement transaction emitted, in log order.
pub struct SettlementTx {
    pub block: u64,
    pub tx_hash: B256,
    pub tx_index: Option<u64>,
    pub settlements: Vec<(u64, Address)>,
    pub trades: Vec<ChainTrade>,
}

pub async fn fetch_logs(
    provider: &impl Provider,
    sources: &[SettlementSource],
    from_block: u64,
    to_block: u64,
) -> Result<Vec<Log>> {
    // Only query the deployments whose active window reaches this range; an
    // empty address set would (per eth_getLogs semantics) match every contract,
    // so bail out early when nothing applies.
    let addresses: Vec<Address> = sources
        .iter()
        .filter(|source| source.overlaps(from_block, to_block))
        .map(|source| source.address)
        .collect();
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let logs = fetch_range(provider, &addresses, from_block, to_block).await?;
    // Drop events an address emitted outside its active window (e.g. the
    // pre-migration contract after it was retired).
    Ok(logs
        .into_iter()
        .filter(|log| {
            let Some(block) = log.block_number else {
                return true;
            };
            sources
                .iter()
                .any(|source| source.address == log.inner.address && source.active_at(block))
        })
        .collect())
}

/// Fetches all logs of `addresses` in `[from, to]`, retrying transient errors
/// with backoff and halving the range whenever the node reports it as too large
/// (too many results / too wide a block range). Sub-ranges are processed with a
/// work stack, so a dense span is subdivided only as far as the node's limit
/// requires; a range that still fails at a single block surfaces the error.
async fn fetch_range(
    provider: &impl Provider,
    addresses: &[Address],
    from: u64,
    to: u64,
) -> Result<Vec<Log>> {
    let mut logs = Vec::new();
    let mut pending = vec![(from, to)];
    while let Some((from, to)) = pending.pop() {
        let filter = Filter::new()
            .address(addresses.to_vec())
            .from_block(from)
            .to_block(to);
        let mut attempt: u32 = 1;
        loop {
            match provider.get_logs(&filter).await {
                Ok(fetched) => {
                    logs.extend(fetched);
                    break;
                }
                Err(err) if is_range_too_large(&err) && from < to => {
                    let mid = from + (to - from) / 2;
                    tracing::info!(
                        from,
                        to,
                        split_at = mid,
                        %err,
                        "getLogs range too large, splitting and retrying the halves"
                    );
                    pending.push((mid + 1, to));
                    pending.push((from, mid));
                    break;
                }
                Err(err) if attempt < MAX_LOG_FETCH_ATTEMPTS => {
                    let delay =
                        (RETRY_BASE_DELAY * 2u32.saturating_pow(attempt - 1)).min(RETRY_MAX_DELAY);
                    tracing::warn!(
                        attempt,
                        max_attempts = MAX_LOG_FETCH_ATTEMPTS,
                        delay_ms = delay.as_millis(),
                        from,
                        to,
                        %err,
                        "getLogs failed, retrying after backoff"
                    );
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("could not fetch logs for {from}..={to} after {attempt} attempts")
                    });
                }
            }
        }
    }
    Ok(logs)
}

/// Groups settlement contract logs by transaction, dropping transactions that
/// did not emit a Settlement event. A transaction may contain multiple
/// settlements (e.g. a settlement calling settle() again in an interaction).
fn group_by_tx(logs: &[Log]) -> Vec<SettlementTx> {
    let mut txs: Vec<SettlementTx> = Vec::new();
    for log in logs {
        let (Some(tx_hash), Some(log_index), Some(block)) =
            (log.transaction_hash, log.log_index, log.block_number)
        else {
            continue;
        };
        let Ok(event) = GPv2SettlementEvents::decode_log(&log.inner) else {
            continue;
        };
        let tx = match txs.iter_mut().find(|tx| tx.tx_hash == tx_hash) {
            Some(tx) => tx,
            None => {
                txs.push(SettlementTx {
                    block,
                    tx_hash,
                    tx_index: log.transaction_index,
                    settlements: Vec::new(),
                    trades: Vec::new(),
                });
                txs.last_mut().unwrap()
            }
        };
        match event.data {
            GPv2SettlementEvents::Settlement(settlement) => {
                tx.settlements.push((log_index, settlement.solver));
            }
            GPv2SettlementEvents::Trade(trade) => {
                tx.trades.push(ChainTrade {
                    log_index,
                    owner: trade.owner,
                    sell_token: trade.sellToken,
                    buy_token: trade.buyToken,
                    sell_amount: trade.sellAmount,
                    buy_amount: trade.buyAmount,
                    fee_amount: trade.feeAmount,
                    order_uid: trade.orderUid.to_vec(),
                });
            }
            _ => (),
        }
    }
    txs.retain(|tx| !tx.settlements.is_empty());
    txs
}

pub async fn fetch_settlements(
    provider: &impl Provider,
    sources: &[SettlementSource],
    from_block: u64,
    to_block: u64,
) -> Result<Vec<SettlementTx>> {
    Ok(group_by_tx(
        &fetch_logs(provider, sources, from_block, to_block).await?,
    ))
}

pub fn offset(block: u64, target_block: u64) -> i64 {
    block.cast_signed() - target_block.cast_signed()
}

/// Formats the distance to the target block, e.g. ", -3" for 3 blocks before
/// it and "" for the target block itself.
pub fn offset_suffix(block: u64, target_block: u64) -> String {
    match offset(block, target_block) {
        0 => String::new(),
        offset => format!(", {offset:+}"),
    }
}

/// All events of a block range as found on the canonical chain, in the shape
/// of their DB tables.
#[derive(Default)]
pub struct CanonicalEvents {
    pub trades: Vec<CanonicalTrade>,
    pub settlements: Vec<CanonicalSettlement>,
    pub invalidations: Vec<CanonicalInvalidation>,
    pub presignatures: Vec<CanonicalPreSignature>,
}

pub struct CanonicalTrade {
    pub block: u64,
    pub log_index: u64,
    pub order_uid: Vec<u8>,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
    pub tx_hash: B256,
}

pub struct CanonicalSettlement {
    pub block: u64,
    pub log_index: u64,
    pub solver: Address,
    pub tx_hash: B256,
}

pub struct CanonicalInvalidation {
    pub block: u64,
    pub log_index: u64,
    pub order_uid: Vec<u8>,
}

pub struct CanonicalPreSignature {
    pub block: u64,
    pub log_index: u64,
    pub owner: Address,
    pub order_uid: Vec<u8>,
    pub signed: bool,
}

pub fn decode_canonical(logs: &[Log]) -> CanonicalEvents {
    let mut events = CanonicalEvents::default();
    for log in logs {
        let (Some(tx_hash), Some(log_index), Some(block)) =
            (log.transaction_hash, log.log_index, log.block_number)
        else {
            continue;
        };
        let Ok(event) = GPv2SettlementEvents::decode_log(&log.inner) else {
            continue;
        };
        match event.data {
            GPv2SettlementEvents::Trade(trade) => events.trades.push(CanonicalTrade {
                block,
                log_index,
                order_uid: trade.orderUid.to_vec(),
                sell_amount: trade.sellAmount,
                buy_amount: trade.buyAmount,
                fee_amount: trade.feeAmount,
                tx_hash,
            }),
            GPv2SettlementEvents::Settlement(settlement) => {
                events.settlements.push(CanonicalSettlement {
                    block,
                    log_index,
                    solver: settlement.solver,
                    tx_hash,
                });
            }
            GPv2SettlementEvents::OrderInvalidated(invalidation) => {
                events.invalidations.push(CanonicalInvalidation {
                    block,
                    log_index,
                    order_uid: invalidation.orderUid.to_vec(),
                });
            }
            GPv2SettlementEvents::PreSignature(presignature) => {
                events.presignatures.push(CanonicalPreSignature {
                    block,
                    log_index,
                    owner: presignature.owner,
                    order_uid: presignature.orderUid.to_vec(),
                    signed: presignature.signed,
                });
            }
            _ => (),
        }
    }
    events.trades.sort_by_key(|e| (e.block, e.log_index));
    events.settlements.sort_by_key(|e| (e.block, e.log_index));
    events.invalidations.sort_by_key(|e| (e.block, e.log_index));
    events.presignatures.sort_by_key(|e| (e.block, e.log_index));
    events
}
