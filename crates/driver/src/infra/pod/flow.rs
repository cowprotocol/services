//! Shadow-mode pod-network bid flow.
//!
//! `PodManager` owns the auction-contract handle, the arbitrator, and the
//! `JoinSet` tracking in-flight pod tasks. `Competition::solve` only calls
//! `PodManager::spawn(...)`. Tasks are aborted when the last `PodManager`
//! clone drops.
//!
//! TODO(production-pod): the fire-and-forget submit + best-effort
//! locked-account recovery is acceptable for shadow mode but must be
//! revisited before pod runs the auction for real. Concretely: queueing
//! semantics for concurrent submissions per solver EOA, retry/backoff
//! policy, observability + alerting on locked-account streaks, and exposing
//! arbitration results for cross-validation against autopilot.

use {
    super::{config, recovery},
    crate::{
        domain::competition::{
            Auction,
            Solved,
            auction::Id,
            solver_winner_selection::{Bid, SolverArbitrator, Unscored},
        },
        infra::{
            api::routes::solve::dto,
            solver::{Account, Solver},
        },
    },
    alloy::network::EthereumWallet,
    anyhow::{Context as _, Result},
    pod_sdk::{
        Provider,
        auctions::client::AuctionClient,
        provider::{PodProvider, PodProviderBuilder},
    },
    std::{
        future::Future,
        sync::{Arc, Mutex},
        time::Duration,
    },
    tokio::task::JoinSet,
    tracing::{Instrument as _, instrument},
    winner_selection::{AuctionContext, state::RankedItem},
};

/// Grace window past the auction deadline allowed for pod RPCs.
const POD_RPC_GRACE: Duration = Duration::from_secs(10);
/// Hard timeout for calls that happen after the auction deadline has
/// already passed (fetching bids, recovering a locked account).
const POD_POST_DEADLINE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct PodManager {
    inner: Arc<PodInner>,
    /// Tracks spawned pod tasks so they are aborted when the last
    /// `PodManager` clone drops. Spawned futures only hold `Arc<PodInner>`,
    /// not `PodManager`, so there is no Arc cycle keeping the `JoinSet`
    /// alive.
    tasks: Arc<Mutex<JoinSet<()>>>,
}

struct PodInner {
    provider: PodProvider,
    arbitrator: SolverArbitrator,
    /// Single `AuctionClient` reused across auctions. AuctionClient just wraps
    /// the alloy contract handle and carries no per-auction state, so building
    /// it once at startup avoids redundant allocations for every bid round.
    client: Arc<AuctionClient>,
}

impl std::fmt::Debug for PodManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodManager").finish_non_exhaustive()
    }
}

impl PodManager {
    /// Build a `PodManager` from solver and pod config. Returns `None` if pod
    /// is unconfigured for this solver. Also returns `None` (after logging)
    /// if the provider cannot be constructed; pod runs in shadow mode and
    /// must not block solver startup on a transient RPC failure.
    pub async fn try_new(
        account: &Account,
        pod_config: &config::Config,
        weth: eth_domain_types::WrappedNativeToken,
    ) -> Option<Self> {
        let provider = match build_pod_provider(account, pod_config).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = %e, "failed to initialize pod provider");
                return None;
            }
        };

        let client = Arc::new(AuctionClient::new(
            provider.clone(),
            pod_config.auction_contract_address,
        ));
        let arbitrator = SolverArbitrator::new(pod_config.max_winners, weth);

        Some(Self {
            inner: Arc::new(PodInner {
                provider,
                arbitrator,
                client,
            }),
            tasks: Arc::new(Mutex::new(JoinSet::new())),
        })
    }

    /// Fire-and-forget shadow-mode flow: submits the proposed solutions to
    /// pod, waits for the auction to end, fetches all bids, and runs local
    /// arbitration. Errors are logged and never surfaced to the caller.
    ///
    /// Bid serialization and [`AuctionContext`] construction happen
    /// synchronously on the calling thread, so the spawned task only owns
    /// cheap data and the [`Auction`] is never deep-cloned. The full
    /// `scored` vec is sent so pod's arbitrator sees the same input set
    /// as autopilot's.
    pub fn spawn(
        &self,
        auction_id: Id,
        deadline: chrono::DateTime<chrono::Utc>,
        scored: Vec<Solved>,
        auction: &Auction,
        solver: Solver,
    ) {
        let bid_value = match scored.first() {
            Some(s) => s.score.0,
            None => return,
        };
        let bid_data = match serde_json::to_vec(&dto::SolveResponse::new(scored, &solver)) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize pod bid payload");
                return;
            }
        };
        let context = AuctionContext::from(auction);

        let inner = self.inner.clone();
        let span =
            tracing::info_span!("pod_flow", auction_id = %auction_id.0, solver = %solver.name());
        let task = async move {
            if let Err(e) = inner
                .submit_bid(auction_id, deadline, bid_value, bid_data, &solver)
                .await
            {
                tracing::warn!(error = %e, "pod bid submission failed (shadow mode)");
                return;
            }

            let participants = match inner.fetch_bids(auction_id, deadline).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "pod fetch bids failed (shadow mode)");
                    return;
                }
            };

            inner.local_arbitration(auction_id, &context, participants);
        };
        self.tasks.lock().unwrap().spawn(task.instrument(span));
    }
}

impl PodInner {
    #[instrument(name = "pod_submit_bid", skip_all, fields(auction_id = %auction_id.0))]
    async fn submit_bid(
        &self,
        auction_id: Id,
        deadline: chrono::DateTime<chrono::Utc>,
        bid_value: pod_sdk::U256,
        bid_data: Vec<u8>,
        solver: &Solver,
    ) -> Result<()> {
        let pod_auction_id =
            pod_sdk::U256::from(u64::try_from(auction_id.0).context("auction id")?);

        tracing::info!(score = %bid_value, payload_len = bid_data.len(), "submitting bid");
        let submit = || async {
            self.client
                .submit_bid(pod_auction_id, deadline.into(), bid_value, bid_data.clone())
                .await
        };

        // `is_account_locked_error` matches the raw pod-sdk error string.
        // `with_timeout` would wrap it via `.context()`, hiding that
        // message (anyhow's `Display` shows only the top context). Handle
        // the timeout inline to keep the raw error visible.
        let timeout = remaining_until(deadline) + POD_RPC_GRACE;
        match tokio::time::timeout(timeout, submit()).await {
            Ok(Ok(_)) => {
                tracing::info!("bid submitted");
                return Ok(());
            }
            Ok(Err(e)) if recovery::is_account_locked_error(&e.to_string()) => {
                tracing::warn!(error = %e, "locked account detected, attempting recovery");
            }
            Ok(Err(e)) => return Err(e.context("submit bid")),
            Err(_) => anyhow::bail!("submit bid timed out after {timeout:?}"),
        }

        if !with_timeout(
            "recover locked account",
            POD_POST_DEADLINE_TIMEOUT,
            recovery::recover_locked_account(&self.provider, solver.address()),
        )
        .await?
        {
            anyhow::bail!("submission failed but account was not locked");
        }

        let timeout = remaining_until(deadline) + POD_RPC_GRACE;
        match tokio::time::timeout(timeout, submit()).await {
            Ok(Ok(_)) => {
                tracing::info!("bid submitted after recovery");
                Ok(())
            }
            Ok(Err(e)) => Err(e.context("submit bid after recovery")),
            Err(_) => anyhow::bail!("submit bid after recovery timed out after {timeout:?}"),
        }
    }

    #[instrument(name = "pod_fetch_bids", skip_all, fields(auction_id = %auction_id.0))]
    async fn fetch_bids(
        &self,
        auction_id: Id,
        deadline: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Bid<Unscored>>> {
        let pod_auction_id =
            pod_sdk::U256::from(u64::try_from(auction_id.0).context("auction id")?);

        let wait_timeout = remaining_until(deadline) + POD_RPC_GRACE;
        with_timeout(
            "wait for auction end",
            wait_timeout,
            self.client.wait_for_auction_end(deadline.into()),
        )
        .await?;

        let bids = with_timeout(
            "fetch bids",
            POD_POST_DEADLINE_TIMEOUT,
            self.client.fetch_bids(pod_auction_id),
        )
        .await?;

        // `bid.bidder` is the on-chain signer; `submission_address` inside
        // `bid.data` is unauthenticated payload that any submitter can set
        // to any value. Override with `bid.bidder` and warn on mismatch so
        // impersonation attempts stay visible. Distinct bidders also keep
        // `SolutionKey`s unique downstream.
        let mut participants = Vec::with_capacity(bids.len());
        let mut malformed = 0;
        let mut spoofed = 0;
        for bid in bids {
            match serde_json::from_slice::<dto::SolveResponse>(&bid.data) {
                Ok(resp) => {
                    for mut solution in resp.solutions {
                        if solution.submission_address != bid.bidder {
                            spoofed += 1;
                            tracing::warn!(
                                bidder = %bid.bidder,
                                claimed = %solution.submission_address,
                                "submission_address mismatch, overriding with on-chain bidder",
                            );
                            solution.submission_address = bid.bidder;
                        }
                        participants.push(Bid::new(solution));
                    }
                }
                Err(e) => {
                    malformed += 1;
                    tracing::warn!(error = %e, bidder = %bid.bidder, "skipping malformed bid");
                }
            }
        }
        if malformed > 0 {
            tracing::warn!(malformed, "some bids were malformed and skipped");
        }
        if spoofed > 0 {
            tracing::warn!(spoofed, "some bids declared a different submission_address");
        }
        tracing::info!(num_participants = participants.len(), "fetched bids");
        Ok(participants)
    }

    #[instrument(
        name = "pod_local_arbitration",
        skip_all,
        fields(auction_id = %auction_id.0, num_participants = participants.len()),
    )]
    fn local_arbitration(
        &self,
        auction_id: Id,
        context: &AuctionContext,
        participants: Vec<Bid<Unscored>>,
    ) {
        // `auction_id` is read by the `#[instrument]` span fields above.
        let _ = auction_id;
        let ranked = self
            .arbitrator
            .arbitrate_with_context(participants, context);

        let (winners, non_winners): (Vec<_>, Vec<_>) = ranked.iter().partition(|b| b.is_winner());
        tracing::info!(
            num_winners = winners.len(),
            num_non_winners = non_winners.len(),
            "local arbitration completed",
        );
        for winner in winners {
            tracing::info!(
                submission_address = %winner.submission_address,
                computed_score = ?winner.score(),
                "winner selected",
            );
        }
    }
}

/// Time remaining until `deadline`. Returns zero if already elapsed.
fn remaining_until(deadline: chrono::DateTime<chrono::Utc>) -> Duration {
    (deadline - chrono::Utc::now())
        .to_std()
        .unwrap_or(Duration::ZERO)
}

/// Wrap a future in a hard timeout. On elapse, returns an `anyhow::Error`
/// labelled with `label`; otherwise propagates the inner result with
/// `label` as added context.
async fn with_timeout<F, T, E>(label: &'static str, dur: Duration, f: F) -> Result<T>
where
    F: Future<Output = std::result::Result<T, E>>,
    anyhow::Error: From<E>,
{
    match tokio::time::timeout(dur, f).await {
        Ok(r) => r.map_err(|e| anyhow::Error::from(e).context(label)),
        Err(_) => Err(anyhow::anyhow!("{label} timed out after {dur:?}")),
    }
}

async fn build_pod_provider(account: &Account, pod_config: &config::Config) -> Result<PodProvider> {
    let wallet = match account {
        Account::PrivateKey(s) => EthereumWallet::from(s.clone()),
        Account::Kms(s) => EthereumWallet::from(s.clone()),
        Account::Address(addr) => {
            anyhow::bail!("address-only account ({addr:?}) cannot sign pod transactions")
        }
    };
    let signer_address = alloy::network::TxSigner::address(account);
    let provider = PodProviderBuilder::with_recommended_settings()
        .wallet(wallet)
        .on_url(pod_config.endpoint.clone())
        .await?;

    // Diagnostic info for debugging pending-TX issues; don't fail provider
    // creation if these RPCs hiccup.
    let balance = provider.get_balance(signer_address).await.ok();
    let nonce = provider.get_transaction_count(signer_address).await.ok();
    tracing::info!(
        %signer_address,
        ?balance,
        ?nonce,
        "pod provider initialized",
    );
    Ok(provider)
}
