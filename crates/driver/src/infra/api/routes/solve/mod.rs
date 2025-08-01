mod dto;

pub use dto::AuctionError;
use {
    crate::{
        domain,
        infra::{
            api::{Error, State},
            observe,
        },
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
        if auction.orders.is_empty() {
            return Err(domain::competition::Error::NoValidOrdersFound.into());
        }
        let result = competition.solve(auction).await;
        // Solving takes some time, so there is a chance for the settlement queue to
        // have capacity again.
        competition.ensure_settle_queue_capacity()?;
        observe::solved(state.solver().name(), &result);
        Ok(axum::Json(dto::SolveResponse::new(
            result?,
            &competition.solver,
        )))
    };

    handle_request
        .instrument(tracing::info_span!("/solve", solver = %state.solver().name(), auction_id))
        .await
}
