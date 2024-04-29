mod dto;

pub use dto::{Auction, AuctionError};
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
    auction: axum::Json<dto::Auction>,
) -> Result<axum::Json<dto::Solved>, (hyper::StatusCode, axum::Json<Error>)> {
    let auction_id = auction.id();
    let handle_request = async {
        observe::auction(auction_id);
        let start = Instant::now();
        let auction = auction.0;
        let domain_auction = auction
            .clone()
            .into_domain(state.eth(), state.tokens(), state.timeouts())
            .await
            .tap_err(|err| {
                observe::invalid_dto(err, "auction");
            })?;
        tracing::debug!(elapsed = ?start.elapsed(), "auction task execution time");
        let domain_auction = state.pre_processor().prioritize(domain_auction).await;
        let competition = state.competition();
        let result = match competition.solve(&domain_auction).await {
            Ok((result, liquidity)) => {
                state
                    .solver()
                    .persistence()
                    .archive_auction(auction, liquidity);
                Ok(result)
            }
            Err(e) => Err(e),
        };
        observe::solved(state.solver().name(), &result);
        Ok(axum::Json(dto::Solved::new(result?, &competition.solver)))
    };

    handle_request
        .instrument(tracing::info_span!("/solve", solver = %state.solver().name(), auction_id))
        .await
}
