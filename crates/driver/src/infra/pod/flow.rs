//! Shadow-mode pod-network bid flow.
//!
//! `PodManager` owns everything that is only meaningful when pod is enabled
//! for a solver: the provider, the auction contract handle, the reusable
//! `AuctionClient`, and the local arbitrator that tallies fetched bids.
//! `Competition::solve` only needs `PodManager::spawn(...)` — the rest of the
//! pod logic is contained here and never leaks into the main competition
//! state machine.
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
    std::sync::Arc,
    tracing::{Instrument as _, instrument},
    winner_selection::state::RankedItem,
};

#[derive(Clone)]
pub struct PodManager {
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
    /// is unconfigured for this solver, and logs and returns `None` if the
    /// provider cannot be constructed (e.g. transient RPC failure at startup
    /// — pod runs in shadow mode and must not block solver startup).
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
            provider,
            arbitrator,
            client,
        })
    }

    /// Fire-and-forget shadow-mode flow: submits the best bid to pod, waits
    /// for the auction to end, fetches all bids, and runs local arbitration.
    /// The caller is not blocked and any error is logged but never surfaced.
    pub fn spawn(
        &self,
        auction_id: Id,
        auction: Auction,
        deadline: chrono::DateTime<chrono::Utc>,
        best: crate::domain::competition::Solved,
        solver: Solver,
    ) {
        let me = self.clone();
        let span =
            tracing::info_span!("pod_flow", auction_id = %auction_id.0, solver = %solver.name());
        tokio::spawn(
            async move {
                if let Err(e) = me.submit_bid(auction_id, deadline, best, &solver).await {
                    tracing::warn!(error = %e, "pod bid submission failed (shadow mode)");
                    return;
                }

                let participants = match me.fetch_bids(auction_id, deadline).await {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!(error = %e, "pod fetch bids failed (shadow mode)");
                        return;
                    }
                };

                if let Err(e) = me.local_arbitration(&auction, auction_id, participants) {
                    tracing::warn!(error = %e, "pod local arbitration failed (shadow mode)");
                }
            }
            .instrument(span),
        );
    }

    #[instrument(name = "pod_submit_bid", skip_all, fields(auction_id = %auction_id.0))]
    async fn submit_bid(
        &self,
        auction_id: Id,
        deadline: chrono::DateTime<chrono::Utc>,
        best: crate::domain::competition::Solved,
        solver: &Solver,
    ) -> Result<()> {
        let pod_auction_id =
            pod_sdk::U256::from(u64::try_from(auction_id.0).context("auction id")?);
        let value = best.score.0;
        let data = serde_json::to_vec(&dto::SolveResponse::new(vec![best], solver))
            .context("serialize bid payload")?;

        tracing::info!(score = %value, payload_len = data.len(), "submitting bid");
        let submit = || async {
            self.client
                .submit_bid(pod_auction_id, deadline.into(), value, data.clone())
                .await
        };

        match submit().await {
            Ok(_) => {
                tracing::info!("bid submitted");
                return Ok(());
            }
            Err(e) if recovery::is_account_locked_error(&e.to_string()) => {
                tracing::warn!(error = %e, "locked account detected, attempting recovery");
            }
            Err(e) => return Err(e.context("submit bid")),
        }

        if !recovery::recover_locked_account(&self.provider, solver.address())
            .await
            .context("recover locked account")?
        {
            anyhow::bail!("submission failed but account was not locked");
        }

        submit().await.context("submit bid after recovery")?;
        tracing::info!("bid submitted after recovery");
        Ok(())
    }

    #[instrument(name = "pod_fetch_bids", skip_all, fields(auction_id = %auction_id.0))]
    async fn fetch_bids(
        &self,
        auction_id: Id,
        deadline: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Bid<Unscored>>> {
        let pod_auction_id =
            pod_sdk::U256::from(u64::try_from(auction_id.0).context("auction id")?);

        self.client
            .wait_for_auction_end(deadline.into())
            .await
            .context("wait for auction end")?;

        let bids = self
            .client
            .fetch_bids(pod_auction_id)
            .await
            .context("fetch bids")?;

        let mut participants = Vec::with_capacity(bids.len());
        let mut malformed = 0;
        for bid in bids {
            match serde_json::from_slice::<dto::SolveResponse>(&bid.data) {
                Ok(resp) => participants.extend(resp.solutions.into_iter().map(Bid::new)),
                Err(e) => {
                    malformed += 1;
                    tracing::warn!(error = %e, bidder = %bid.bidder, "skipping malformed bid");
                }
            }
        }
        if malformed > 0 {
            tracing::warn!(malformed, "some bids were malformed and skipped");
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
        auction: &Auction,
        auction_id: Id,
        participants: Vec<Bid<Unscored>>,
    ) -> Result<()> {
        let ranked = self.arbitrator.arbitrate(participants, auction);

        let (winners, non_winners): (Vec<_>, Vec<_>) = ranked.iter().partition(|b| b.is_winner());
        tracing::info!(
            num_winners = winners.len(),
            num_non_winners = non_winners.len(),
            "local arbitration completed",
        );
        for winner in winners {
            tracing::info!(
                submission_address = %winner.submission_address(),
                computed_score = ?winner.score(),
                "winner selected",
            );
        }
        Ok(())
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
