pub mod balancer_v2;
pub mod uniswap_v3;

use {
    alloy_primitives::{Address, B256},
    alloy_provider::Provider,
    alloy_rpc_types_eth::{Filter, FilterSet, Log},
    alloy_transport::RpcError,
    anyhow::Result,
    contracts::ERC20,
    ethrpc::{AlloyProvider, alloy::errors::ContractErrorExt},
    std::time::Duration,
};

/// Retries `f` while the error is a transient transport failure
/// (`is_node_error`). Contract reverts and decoding failures bail out
/// immediately. On giveup, hands `on_giveup` the accumulated errors and
/// returns `None`.
pub(crate) async fn retry_node_call<T, Fut>(
    f: impl Fn() -> Fut,
    on_giveup: impl FnOnce(&[alloy_contract::Error]),
) -> Option<T>
where
    Fut: std::future::Future<Output = Result<T, alloy_contract::Error>>,
{
    match shared::retry::retry_with_sleep_if(f, |err: &alloy_contract::Error| err.is_node_error())
        .await
    {
        Ok(v) => Some(v),
        Err(errors) => {
            on_giveup(&errors);
            None
        }
    }
}

/// Fetches a token's `decimals()`, retrying transient node errors. Returns
/// `None` if the call ultimately fails (revert / decode / giveup).
pub(crate) async fn fetch_decimals(provider: &AlloyProvider, token: Address) -> Option<u8> {
    retry_node_call(
        || async move {
            ERC20::Instance::new(token, provider.clone())
                .decimals()
                .call()
                .await
        },
        |errors| tracing::warn!(%token, ?errors, "fetch_decimals gave up"),
    )
    .await
}

/// True if the server-side JSON-RPC payload rejected `eth_getLogs` for
/// being too wide / returning too many logs / exceeding a response-size
/// cap / hitting the server's query timeout. Substrings cover the
/// rejections empirically seen on OVH and Alchemy mainnet. Transport-level
/// errors (HTTP timeouts, DNS, connection resets) live in other `RpcError`
/// variants and short-circuit to false, so client-side noise can't trigger
/// pointless bisection.
pub(crate) fn is_range_too_large(err: &alloy_transport::TransportError) -> bool {
    let RpcError::ErrorResp(payload) = err else {
        return false;
    };
    let msg = payload.message.to_lowercase();
    msg.contains("max block range")
        || msg.contains("max results")
        || msg.contains("log response size exceeded")
        || msg.contains("query timeout exceeded")
        || msg.contains("response is too big")
}

/// Bisecting bound — substring matching on RPC error messages is necessarily
/// approximate, and a misclassified error would otherwise burn `log2(range)`
/// RPC calls before the recursion bottoms out at `to == from`. 8 halvings =
/// 256× resolution; for the indexer's ~1k-block chunks that means giving up
/// around ~4-block ranges, well past where range-size could plausibly still
/// be the cause.
const MAX_BISECTION_DEPTH: u32 = 8;

/// Retry transient `eth_getLogs` failures (timeout, reset, throttle) with
/// backoff, capped by a per-call timeout, so one blip can't abort a long
/// cold-seed scan. Range-size rejections are bisected, not retried.
const GETLOGS_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_GETLOGS_RETRIES: u32 = 6;
const GETLOGS_RETRY_BACKOFF: Duration = Duration::from_secs(2);

/// Fetches logs for `[from, to]` filtered by the given contract addresses
/// and `topic0` event signatures, sequentially bisecting the block range on
/// "too large" rejections until each sub-range is tractable. An empty
/// `addresses` list means "any contract". Bisection depth is capped by
/// [`MAX_BISECTION_DEPTH`].
pub(crate) fn bisecting_get_logs(
    provider: &AlloyProvider,
    from: u64,
    to: u64,
    addresses: Vec<Address>,
    topics: Vec<B256>,
) -> futures::future::BoxFuture<'_, Result<Vec<Log>>> {
    bisecting_get_logs_with_depth(provider, from, to, addresses, topics, 0)
}

fn bisecting_get_logs_with_depth(
    provider: &AlloyProvider,
    from: u64,
    to: u64,
    addresses: Vec<Address>,
    topics: Vec<B256>,
    depth: u32,
) -> futures::future::BoxFuture<'_, Result<Vec<Log>>> {
    Box::pin(async move {
        let filter = Filter::new()
            .address(addresses.clone())
            .event_signature(FilterSet::from_iter(topics.clone()))
            .from_block(from)
            .to_block(to);

        let mut attempt = 0u32;
        loop {
            let err = match tokio::time::timeout(GETLOGS_TIMEOUT, provider.get_logs(&filter)).await
            {
                Ok(Ok(logs)) => return Ok(logs),
                // Range-size rejection: bisect (below), not retried.
                Ok(Err(err)) if is_range_too_large(&err) => {
                    if to <= from || depth >= MAX_BISECTION_DEPTH {
                        return Err(
                            anyhow::Error::new(err).context(format!("get_logs({from}..={to})"))
                        );
                    }
                    break;
                }
                Ok(Err(err)) => anyhow::Error::new(err).context(format!("get_logs({from}..={to})")),
                Err(_elapsed) => anyhow::anyhow!("get_logs({from}..={to}) timed out"),
            };
            // Transient failure: retry with backoff, then give up.
            if attempt >= MAX_GETLOGS_RETRIES {
                return Err(err);
            }
            attempt += 1;
            tracing::warn!(%err, attempt, from, to, "get_logs failed, retrying");
            tokio::time::sleep(GETLOGS_RETRY_BACKOFF * attempt).await;
        }

        let mid = (from + to) / 2;
        tracing::debug!(from, to, mid, depth, "range too large, bisecting");
        let mut left = bisecting_get_logs_with_depth(
            provider,
            from,
            mid,
            addresses.clone(),
            topics.clone(),
            depth + 1,
        )
        .await?;
        let right =
            bisecting_get_logs_with_depth(provider, mid + 1, to, addresses, topics, depth + 1)
                .await?;
        left.extend(right);
        Ok(left)
    })
}
