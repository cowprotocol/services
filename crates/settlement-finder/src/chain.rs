//! Fetching and decoding settlement contract logs from the chain.

use {
    alloy_primitives::{Address, B256, U256},
    alloy_provider::Provider,
    alloy_rpc_types::{Filter, FilterSet, Log},
    alloy_sol_types::{SolEvent, SolEventInterface},
    alloy_transport::{RpcError, TransportErrorKind},
    anyhow::{Context, Result},
    contracts::GPv2Settlement::GPv2Settlement::{self as GPv2, GPv2SettlementEvents},
    std::{collections::BTreeMap, fmt, time::Duration},
};

/// getLogs is idempotent and read-only, so transient failures (connection
/// resets, body-decode errors, provider rate limits like Alchemy's 429) are
/// safe to retry. We back off exponentially and only surface an error once the
/// attempts are exhausted; a genuinely permanent error (e.g. a bad block range)
/// still fails, just after a bounded delay.
const MAX_LOG_FETCH_ATTEMPTS: u32 = 8;
const RETRY_BASE_DELAY: Duration = Duration::from_millis(200);
const RETRY_MAX_DELAY: Duration = Duration::from_secs(10);

/// Per-attempt ceiling for a single RPC call, so a hung connection (a stalled
/// body read, a half-open socket) is abandoned and retried rather than
/// blocking the scan forever. Kept short: a healthy `getLogs` returns well
/// within it, and on the multi-block range path a timeout is treated as a
/// signal that the batch is too big and the range is split ASAP.
const RPC_CALL_TIMEOUT: Duration = Duration::from_secs(5);

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

/// Whether an RPC error is a rate limit. Checked before `is_range_too_large`
/// because messages like "rate limit exceeded" also contain its needles, and
/// the right response to rate limiting is backing off, not splitting the range
/// and doubling the request volume.
fn is_rate_limited(err: &impl fmt::Display) -> bool {
    let message = err.to_string().to_lowercase();
    ["rate limit", "too many requests", "429"]
        .iter()
        .any(|needle| message.contains(needle))
}

/// Whether an error looks like an oversized response the node could not
/// deliver, as opposed to a clean "query too large" message. A batch that is
/// too big often does not come back as a structured error: the node starts
/// streaming a huge body and the connection breaks mid-flight ("error decoding
/// response body", "error reading a body from connection", HTTP/2 "stream
/// error"), or a proxy rejects the payload (HTTP 413). Retrying the same range
/// just reproduces it; the fix is to split the range, same as
/// `is_range_too_large`.
fn is_undeliverable_response(err: &RpcError<TransportErrorKind>) -> bool {
    if let RpcError::Transport(TransportErrorKind::HttpError(http)) = err
        && http.status == 413
    {
        return true;
    }
    let message = err.to_string().to_lowercase();
    [
        "error decoding response body",
        "error reading a body",
        "stream error",
        "incomplete message",
        "connection reset",
        "connection closed before message completed",
        "broken pipe",
        "body limit",
        "frame size",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn backoff_delay(attempt: u32) -> Duration {
    (RETRY_BASE_DELAY * 2u32.saturating_pow(attempt - 1)).min(RETRY_MAX_DELAY)
}

/// A short, loggable description of what the node returned, so a retry line
/// distinguishes an HTTP 429/5xx from a JSON-RPC error from a body/stream
/// failure that never reached a status.
fn request_status(err: &RpcError<TransportErrorKind>) -> String {
    if let Some(resp) = err.as_error_resp() {
        return format!("json-rpc code {}", resp.code);
    }
    match err {
        RpcError::Transport(TransportErrorKind::HttpError(http)) => format!("http {}", http.status),
        RpcError::Transport(TransportErrorKind::BackendGone) => "backend gone".to_owned(),
        RpcError::Transport(_) => "transport error (no status)".to_owned(),
        RpcError::NullResp => "null response".to_owned(),
        RpcError::DeserError { .. } => "response decode error".to_owned(),
        RpcError::SerError(_) => "request encode error".to_owned(),
        _ => "no status".to_owned(),
    }
}

/// Runs an idempotent RPC call with a per-attempt timeout, retrying transient
/// failures (connection resets, mid-body stream errors, timeouts, provider
/// rate limits) with exponential backoff. Only the terminal error is surfaced,
/// after the attempts are exhausted. Each retry logs the request status (HTTP
/// code, JSON-RPC code, or failure kind). `op` is re-invoked per attempt, so it
/// must build a fresh future each time (own its inputs).
async fn with_retries<T, F, Fut>(what: &str, mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, RpcError<TransportErrorKind>>>,
{
    let mut attempt: u32 = 1;
    loop {
        let (status, err): (String, anyhow::Error) =
            match tokio::time::timeout(RPC_CALL_TIMEOUT, op()).await {
                Ok(Ok(value)) => return Ok(value),
                Ok(Err(err)) => (request_status(&err), err.into()),
                Err(_elapsed) => (
                    "timeout".to_owned(),
                    anyhow::anyhow!("timed out after {}s", RPC_CALL_TIMEOUT.as_secs()),
                ),
            };
        if attempt >= MAX_LOG_FETCH_ATTEMPTS {
            return Err(err)
                .with_context(|| format!("{what} failed after {attempt} attempts ({status})"));
        }
        let delay = backoff_delay(attempt);
        tracing::warn!(
            attempt,
            max_attempts = MAX_LOG_FETCH_ATTEMPTS,
            delay_ms = delay.as_millis(),
            what,
            status,
            %err,
            "rpc call failed, retrying after backoff"
        );
        tokio::time::sleep(delay).await;
        attempt += 1;
    }
}

/// The topic0 filter for the event kinds the tool decodes. The settlement
/// contract also emits Interaction events, which would only inflate responses
/// (raising the odds of provider result caps) to be discarded after decoding.
fn event_topics() -> FilterSet<B256> {
    FilterSet::from_iter([
        GPv2::Settlement::SIGNATURE_HASH,
        GPv2::Trade::SIGNATURE_HASH,
        GPv2::OrderInvalidated::SIGNATURE_HASH,
        GPv2::PreSignature::SIGNATURE_HASH,
    ])
}

/// Rejects impossible getLogs responses instead of silently ingesting them:
/// drops logs the node marked as removed (reorged away) and fails when two
/// logs claim the same block number under different block hashes, which means
/// the node's log index is serving a mix of block variants (observed on public
/// Gnosis endpoints, where it manifested as shifted log indices and mass false
/// mismatches).
fn sanitize_logs(logs: Vec<Log>) -> Result<Vec<Log>> {
    let logs: Vec<Log> = logs
        .into_iter()
        .filter(|log| {
            if log.removed {
                tracing::warn!(
                    block = log.block_number,
                    log_index = log.log_index,
                    "dropping a log the node marked as removed (reorged away)"
                );
            }
            !log.removed
        })
        .collect();
    let mut hash_by_block: BTreeMap<u64, B256> = BTreeMap::new();
    for log in &logs {
        let (Some(block), Some(hash)) = (log.block_number, log.block_hash) else {
            continue;
        };
        if let Some(previous) = hash_by_block.insert(block, hash)
            && previous != hash
        {
            anyhow::bail!(
                "getLogs returned logs of two different variants of block {block} ({previous} and \
                 {hash}); the node's log index is inconsistent"
            );
        }
    }
    Ok(logs)
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
    let logs = sanitize_logs(fetch_range(provider, &addresses, from_block, to_block).await?)?;
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

/// Fetches one block's settlement contract logs via an EIP-234 blockHash
/// query. This bypasses the node's block-number log index — the part observed
/// to serve wrong data (logs of a different block variant under the queried
/// number) — and pins the response to the given canonical hash, so it is the
/// authoritative way to double-check a block that a range query made look
/// damaged.
pub async fn fetch_logs_by_block_hash(
    provider: &impl Provider,
    sources: &[SettlementSource],
    block: u64,
    block_hash: B256,
) -> Result<Vec<Log>> {
    let addresses: Vec<Address> = sources
        .iter()
        .filter(|source| source.active_at(block))
        .map(|source| source.address)
        .collect();
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let filter = Filter::new()
        .address(addresses)
        .event_signature(event_topics())
        .at_block_hash(block_hash);
    let logs = with_retries(&format!("getLogs of block {block} by hash"), || {
        let filter = filter.clone();
        async move { provider.get_logs(&filter).await }
    })
    .await?;
    sanitize_logs(logs)
}

/// The canonical hash of a block, or `None` if the node has no such block.
///
/// Uses a raw `eth_getBlockByNumber` and reads only the `hash` field rather
/// than the typed `get_block_by_number`, whose header deserialization rejects
/// the non-Ethereum fields of PoA chains: Gnosis (OpenEthereum) blocks carry
/// `nonce: null` plus `step`/`signature`, which make alloy's strict `Header`
/// fail with "invalid type: null, expected 8 bytes". Only the hash is needed
/// here, so the rest of the header is never parsed.
pub async fn canonical_block_hash(provider: &impl Provider, block: u64) -> Result<Option<B256>> {
    let params = (format!("0x{block:x}"), false);
    let response: Option<serde_json::Value> = with_retries(&format!("fetch block {block}"), || {
        let params = params.clone();
        async move {
            provider
                .raw_request("eth_getBlockByNumber".into(), params)
                .await
        }
    })
    .await?;
    let Some(response) = response else {
        return Ok(None);
    };
    let hash = response
        .get("hash")
        .and_then(|hash| hash.as_str())
        .with_context(|| format!("block {block} response has no hash field"))?;
    let hash = hash
        .parse::<B256>()
        .with_context(|| format!("block {block} hash {hash} is not a valid B256"))?;
    Ok(Some(hash))
}

/// Verifies that every block's logs carry that block's canonical hash,
/// fetching each distinct block's header once. `sanitize_logs` already
/// rejected intra-response disagreements; this catches the harder failure
/// where the node consistently serves another block's logs under the queried
/// number. Anything that writes chain data to the DB must pass this first.
pub async fn validate_canonical_hashes(provider: &impl Provider, logs: &[Log]) -> Result<()> {
    let mut hash_by_block: BTreeMap<u64, B256> = BTreeMap::new();
    for log in logs {
        let (Some(block), Some(hash)) = (log.block_number, log.block_hash) else {
            anyhow::bail!("cannot validate a log without a block number and block hash: {log:?}");
        };
        hash_by_block.insert(block, hash);
    }
    for (&block, &hash) in &hash_by_block {
        let canonical = canonical_block_hash(provider, block)
            .await?
            .with_context(|| format!("node has no block {block}"))?;
        anyhow::ensure!(
            canonical == hash,
            "logs of block {block} carry hash {hash} but the canonical block is {canonical}; the \
             node's log index is serving another block's logs under this number"
        );
    }
    Ok(())
}

/// Fetches all logs of `addresses` in `[from, to]`, retrying transient errors
/// with backoff and halving the range whenever the batch is too large — when
/// the node says so outright (too many results / too wide a range), when an
/// oversized response fails to deliver (a broken body stream, an HTTP 413), and
/// when a multi-block request times out (a big batch the node cannot return
/// within `RPC_CALL_TIMEOUT`). Sub-ranges are processed with a work stack, so a
/// dense span is subdivided only as far as the node's limit requires; a range
/// that still fails at a single block surfaces the error.
async fn fetch_range(
    provider: &impl Provider,
    addresses: &[Address],
    from: u64,
    to: u64,
) -> Result<Vec<Log>> {
    let split = |pending: &mut Vec<(u64, u64)>, from: u64, to: u64| -> u64 {
        let mid = from + (to - from) / 2;
        pending.push((mid + 1, to));
        pending.push((from, mid));
        mid
    };
    let mut logs = Vec::new();
    let mut pending = vec![(from, to)];
    while let Some((from, to)) = pending.pop() {
        let filter = Filter::new()
            .address(addresses.to_vec())
            .event_signature(event_topics())
            .from_block(from)
            .to_block(to);
        let mut attempt: u32 = 1;
        loop {
            let (status, err): (String, anyhow::Error) =
                match tokio::time::timeout(RPC_CALL_TIMEOUT, provider.get_logs(&filter)).await {
                    Ok(Ok(fetched)) => {
                        logs.extend(fetched);
                        break;
                    }
                    // A timeout on a multi-block range almost always means the
                    // batch is too big to return in time, so split ASAP rather
                    // than burning retries on the same doomed range.
                    Err(_elapsed) if from < to => {
                        let mid = split(&mut pending, from, to);
                        tracing::info!(
                            from,
                            to,
                            split_at = mid,
                            status = "timeout",
                            "getLogs timed out, splitting the range and retrying the halves"
                        );
                        break;
                    }
                    Ok(Err(err))
                        if from < to
                            && !is_rate_limited(&err)
                            && (is_range_too_large(&err) || is_undeliverable_response(&err)) =>
                    {
                        let mid = split(&mut pending, from, to);
                        tracing::info!(
                            from,
                            to,
                            split_at = mid,
                            status = request_status(&err),
                            %err,
                            "getLogs batch too large, splitting the range and retrying the halves"
                        );
                        break;
                    }
                    Err(_elapsed) => (
                        "timeout".to_owned(),
                        anyhow::anyhow!("timed out after {}s", RPC_CALL_TIMEOUT.as_secs()),
                    ),
                    Ok(Err(err)) => (request_status(&err), err.into()),
                };
            if attempt >= MAX_LOG_FETCH_ATTEMPTS {
                return Err(err).with_context(|| {
                    format!(
                        "could not fetch logs for {from}..={to} after {attempt} attempts \
                         ({status})"
                    )
                });
            }
            let delay = backoff_delay(attempt);
            tracing::warn!(
                attempt,
                max_attempts = MAX_LOG_FETCH_ATTEMPTS,
                delay_ms = delay.as_millis(),
                from,
                to,
                status,
                %err,
                "getLogs failed, retrying after backoff"
            );
            tokio::time::sleep(delay).await;
            attempt += 1;
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
            tracing::warn!(
                ?log,
                "skipping a log without tx hash, log index or block number"
            );
            continue;
        };
        let event = match GPv2SettlementEvents::decode_log(&log.inner) {
            Ok(event) => event,
            Err(err) => {
                tracing::warn!(
                    block,
                    log_index,
                    %err,
                    "skipping an undecodable settlement contract log"
                );
                continue;
            }
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
            tracing::warn!(
                ?log,
                "skipping a log without tx hash, log index or block number"
            );
            continue;
        };
        let event = match GPv2SettlementEvents::decode_log(&log.inner) {
            Ok(event) => event,
            Err(err) => {
                tracing::warn!(
                    block,
                    log_index,
                    %err,
                    "skipping an undecodable settlement contract log"
                );
                continue;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_response_failures_are_split() {
        // The exact failure shapes seen from Alchemy / HTTP/2 nodes when a
        // batch is too big to stream back.
        for message in [
            "error decoding response body for url (https://node/v2/key)",
            "error reading a body from connection",
            "stream error received: unexpected internal error encountered",
        ] {
            let err = TransportErrorKind::custom_str(message);
            assert!(is_undeliverable_response(&err), "{message}");
        }
    }

    #[test]
    fn unrelated_errors_are_not_split() {
        let err = TransportErrorKind::custom_str("execution reverted");
        assert!(!is_undeliverable_response(&err));
        assert!(!is_range_too_large(&err));
    }
}
