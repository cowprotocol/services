//! Quote decisions for orders created on chain. The orderbook never placed
//! such an order, so nothing linked a quote to it: the indexer stores it
//! waiting, and the task here decides its quote the way placement decides a
//! sponsored order's.

use {
    super::{find_or_create_quote, metrics},
    crate::infra::{api::State, db},
    database::byte_array::ByteArray,
    futures::{StreamExt, stream::FuturesUnordered},
    leader_lock::{LeaderLockTracker, LeaderPgLock},
    std::time::Duration,
    tokio::time::MissedTickBehavior,
    tokio_util::sync::CancellationToken,
};

/// How many orders are quoted at once, the number the EVM autopilot quotes
/// of its on-chain orders.
const MAX_CONCURRENT_QUOTES: usize = 10;

/// Name of the Postgres advisory lock that picks the one orderbook quoting
/// among those sharing a database.
const LEADER_LOCK_KEY: &str = "solana_on_chain_quotes";

/// How an order's quote decision ended.
#[derive(Clone, Copy, Debug)]
pub(super) enum Outcome {
    /// A stored or fresh quote was linked.
    Linked,
    /// The order has no quote: no route, no native price, or costs above
    /// its amount.
    NoQuote,
    /// The quote failed, so the order was decided without one.
    Failed,
    /// Writing the decision failed, so the order still waits for one.
    WriteFailed,
    /// The autopilot stopped waiting first, so nothing was written.
    TooLate,
}

impl Outcome {
    pub(super) const ALL: [Self; 5] = [
        Self::Linked,
        Self::NoQuote,
        Self::Failed,
        Self::WriteFailed,
        Self::TooLate,
    ];

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Linked => "linked",
            Self::NoQuote => "no_quote",
            Self::Failed => "failed",
            Self::WriteFailed => "write_failed",
            Self::TooLate => "too_late",
        }
    }
}

/// Decide the quote of every order created on chain until `shutdown`: each
/// `poll_interval` take the oldest waiting orders, up to
/// [`MAX_CONCURRENT_QUOTES`] quoted at once. An order being quoted is not
/// taken again. Of the orderbooks sharing a database only the holder of the
/// leader lock takes orders, so each order is quoted once.
pub async fn quote_on_chain_orders(
    state: State,
    poll_interval: Duration,
    shutdown: CancellationToken,
) {
    let mut leader = LeaderLockTracker::new(Some(LeaderPgLock::spawn(
        state.pool(),
        LEADER_LOCK_KEY.to_string(),
    )));
    let mut quoting = FuturesUnordered::new();
    let mut in_flight: Vec<ByteArray<32>> = Vec::new();
    let mut polls = tokio::time::interval(poll_interval);
    polls.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            Some(uid) = quoting.next() => in_flight.retain(|quoted| *quoted != uid),
            _ = polls.tick() => {
                if !leader.update() {
                    continue;
                }
                let free = MAX_CONCURRENT_QUOTES - in_flight.len();
                if free == 0 {
                    continue;
                }
                let now = chrono::Utc::now().timestamp();
                let waiting =
                    match db::quote_pending_orders(state.pool(), now, &in_flight, free).await {
                        Ok(waiting) => waiting,
                        Err(err) => {
                            tracing::warn!(?err, "failed to read the orders awaiting a quote");
                            continue;
                        }
                    };
                for pending in waiting {
                    let uid = pending.order.uid;
                    in_flight.push(uid);
                    let state = state.clone();
                    // A task of its own, so the decision goes on while this
                    // loop reads the next waiting orders.
                    let decision = tokio::spawn(async move { decide(&state, pending).await });
                    quoting.push(async move {
                        if let Err(err) = decision.await {
                            tracing::error!(
                                uid = %const_hex::encode_prefixed(uid.0),
                                ?err,
                                "quote decision did not finish"
                            );
                        }
                        uid
                    });
                }
            }
        }
    }
    // Frees the lock at once, so a standby takes over while this orderbook
    // drains its requests.
    leader.release().await;
}

/// Decide one order's quote and store the decision. An order whose quote
/// fails is decided without one: it exists on chain and cannot be refused.
async fn decide(state: &State, pending: db::PendingOrder) {
    let db::PendingOrder {
        order,
        created,
        quote_id,
    } = pending;
    let uid = const_hex::encode_prefixed(order.uid.0);
    let (quote, failed) = match find_or_create_quote(state, &order, quote_id, created).await {
        Ok(quote) => (quote, false),
        Err(failure) => {
            tracing::warn!(%uid, ?failure, "quote failed, deciding the order without one");
            (None, true)
        }
    };
    // A failed write leaves the order waiting, so a later poll quotes it
    // again until the autopilot stops waiting for it.
    let outcome = match db::decide_quote(state.pool(), order.uid, quote.as_ref()).await {
        Ok(true) if failed => Outcome::Failed,
        Ok(true) if quote.is_some() => Outcome::Linked,
        Ok(true) => Outcome::NoQuote,
        Ok(false) => Outcome::TooLate,
        Err(err) => {
            tracing::warn!(%uid, ?err, "quote decision write failed");
            Outcome::WriteFailed
        }
    };
    tracing::debug!(
        %uid,
        ?outcome,
        quote_id = ?quote.map(|quote| quote.quote_id),
        "decided the quote of an order created on chain"
    );
    metrics()
        .on_chain_order_quotes
        .with_label_values(&[outcome.as_str()])
        .inc();
}
