mod dto;

pub use dto::AuctionError;
use crate::infra::pod;

use {
    crate::infra::{
        api::{Error, State},
        observe,
    },
    std::time::Instant,
    tap::TapFallible,
    tracing::Instrument,
};

pub(in crate::infra::api) fn solve(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/solve", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    req: axum::Json<dto::SolveRequest>,
) -> Result<axum::Json<dto::SolveResponse>, (hyper::StatusCode, axum::Json<Error>)> {
    let auction_id = req.id();
    let deadline = req.deadline().timestamp();
    let handle_request = async {
        observe::auction(auction_id);
        let start = Instant::now();

        let auction = req
            .0
            .into_domain(state.eth(), state.tokens(), state.timeouts())
            .await
            .tap_err(|err| {
                observe::invalid_dto(err, "auction");
            })?;
        tracing::debug!(elapsed = ?start.elapsed(), "auction task execution time");
        let competition = state.competition();
        let auction = state
            .pre_processor()
            .prioritize(auction, &competition.solver.account().address())
            .await;
        let result = competition.solve(auction).await;
        competition.ensure_settle_queue_capacity()?;
        observe::solved(state.solver().name(), &result);
        let score = if let Ok(Some(sol)) = &result {
            Some(sol.score.0.clone())
        } else {
            None
        };
        let response = dto::SolveResponse::new(result?, &competition.solver);
        if let (Some(score), Some(pod)) = (score, state.pod()) {
            let solver = state.solver().clone();
            let solution_data = serde_json::to_string(&response).expect("failed to serialize solution");
            let pod = pod.clone();
            tokio::spawn(async move {
                let result: Result<(), pod::Error> = async {
                    let deadline = u64::try_from(deadline).map_err(|_| pod::Error::InvalidDeadline(deadline))?;
                    let auction_id = u64::try_from(auction_id).map_err(|_| pod::Error::InvalidAuctionId(auction_id))?;

                    let tx_hash = pod.bid(
                        solver.account(),
                        auction_id,
                        deadline,
                        score,
                        solution_data.as_bytes(),
                    )
                    .await?;

                    tracing::debug!("submitted tx to pod: {:?}", tx_hash);

                    pod.wait_past_perfect(deadline).await?;
                    tracing::debug!("auction ended");

                    let bids = pod.fetch_bids(auction_id, deadline).await?;
                    tracing::debug!("fetched bids: {:?}", bids);

                    // the txhash is used to determine the winner if there are multiple bids with the same value
                    let max_bid = bids.iter().max_by_key(|b| (b.data.value, b.meta.as_ref().unwrap().transaction_hash));
                    match max_bid {
                        Some(max_bid) => {
                            tracing::debug!(
                                "Max log({}): {:?} {}",
                                solver.name(), max_bid.data.bidder, max_bid.data.value
                            );

                            if max_bid.data.bidder == solver.address().into() {
                               tracing::debug!("I won the auction ({})", solver.name());
                            } else {
                               tracing::debug!("I lost the auction ({})", solver.name());
                            }
                        }
                        None => {
                            tracing::debug!("No bids found for auction: {}", auction_id);
                        }
                    }
                    Ok(())
                }
                .await;

                if let Err(err) = result {
                    tracing::debug!("pod error: {:?}", err);
                }
            });
        }
        Ok(axum::Json(response))
    };

    handle_request
        .instrument(tracing::info_span!("/solve", solver = %state.solver().name(), auction_id))
        .await
}
