pub mod dto;

pub use dto::AuctionError;
use {
    crate::{
        domain::{
            self,
            competition::{Auction, Solved},
        },
        infra::{
            api::{Error, State},
            observe,
            pod,
        },
    },
    std::sync::Arc,
    tracing::Instrument,
};

pub(in crate::infra::api) fn solve(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/solve", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    // take the request body as a raw string to delay parsing as much
    // as possible because many requests don't have to be parsed at all
    req: String,
) -> Result<axum::Json<dto::SolveResponse>, (hyper::StatusCode, axum::Json<Error>)> {
    let handle_request = async {
        let competition = state.competition();
        // Auction is needed later for pod
        let (result, auction): (
            Result<Option<Solved>, domain::competition::Error>,
            Option<Auction>,
        ) = match competition.solve(Arc::new(req)).await {
            Ok((solved, auction)) => (Ok(solved), Some(auction)),
            Err(e) => (Err(e), None),
        };
        // Solving takes some time, so there is a chance for the settlement queue to
        // have capacity again.
        competition.ensure_settle_queue_capacity()?;
        observe::solved(state.solver().name(), &result);
        let score = if let Ok(Some(sol)) = &result {
            Some(sol.score.0)
        } else {
            None
        };
        let solve_response = dto::SolveResponse::new(result?, &competition.solver);
        if let (Some(score), Some(pod), Some(auction)) = (score, state.pod(), auction) {
            let solution_data =
                serde_json::to_string(&solve_response).expect("failed to serialize solution");
            let auction_id = auction.id.unwrap().0;
            let deadline = auction.deadline.timestamp_micros() as u64;

            let solver = state.solver().clone();
            let pod = pod.clone();

            tokio::spawn(async move {
                let result: Result<(), pod::api::Error> = async {
                    let tx_hash = pod
                        .bid(
                            solver.account(),
                            auction_id,
                            deadline,
                            score,
                            solution_data.as_bytes(),
                        )
                        .await?;

                    tracing::debug!("[pod] submitted tx to pod: {:?}", tx_hash);

                    pod.wait_past_perfect(deadline).await?;
                    tracing::debug!("[pod] auction ended. ppt has been reached");

                    let bids = pod.fetch_bids(auction_id, deadline).await?;
                    tracing::debug!("[pod] fetched bids: {:?}", bids.len());

                    // Dummy demo for selecting winning bid.
                    let max_bid = bids.iter().max_by_key(|b| b.data.value);
                    match max_bid {
                        Some(max_bid) => {
                            tracing::debug!(
                                "[pod] Max log({}): {:?} {}",
                                solver.name(),
                                max_bid.data.bidder,
                                max_bid.data.value
                            );

                            if max_bid.address.0.0 == solver.address().0.0 {
                                tracing::debug!("[pod] I won the auction ({})", solver.name());
                            } else {
                                tracing::debug!("[pod] I lost the auction ({})", solver.name());
                            }
                        }
                        None => {
                            tracing::debug!("[pod] No bids found for auction: {}", auction_id);
                        }
                    }

                    Ok(())
                }
                .await;

                if let Err(e) = result {
                    tracing::error!("[pod] error: {:?}", e);
                }
            });
        }
        Ok(axum::Json(solve_response))
    };

    handle_request
        .instrument(tracing::info_span!("/solve", solver = %state.solver().name(), auction_id = tracing::field::Empty))
        .await
}
