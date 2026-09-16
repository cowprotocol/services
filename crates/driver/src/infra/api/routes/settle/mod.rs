mod dto;

use {
    crate::{
        domain::{
            competition::{auction, solution},
            quote,
        },
        infra::{
            api::{self, Error, State, extract::LoggingJson},
            observe,
        },
    },
    tracing::Instrument,
};

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    LoggingJson(req): LoggingJson<dto::SettleRequest>,
) -> Result<(), (axum::http::StatusCode, axum::Json<Error>)> {
    let auction_id =
        auction::Id::try_from(req.auction_id()).map_err(api::routes::AuctionError::from)?;
    let submission_deadline = req.submission_deadline_latest_block();
    let solver = state.solver().name().to_string();

    async move {
        observe::settling();
        let solution_id = match req {
            dto::SettleRequest::Auction(req) => req.solution_id,
            // A fast-path settlement references the quote cached by `/quote`;
            // re-encoding it against the real order yields the solution to
            // settle.
            dto::SettleRequest::FastPath(req) => {
                let req = *req;
                let order = req.order.into_domain(None);
                let limit_prices = solution::LimitPrices {
                    sell: req.limit_prices.sell,
                    buy: req.limit_prices.buy,
                };
                state
                    .competition()
                    .reencode_quote_solution(
                        auction_id,
                        quote::Id(req.quote_id),
                        order,
                        limit_prices,
                    )
                    .await?
            }
        };
        let result = state
            .competition()
            .settle(auction_id, solution_id, submission_deadline.into())
            .await;
        result.map(|_| ()).map_err(Into::into)
    }
    .instrument(tracing::info_span!("/settle", solver, %auction_id))
    .await
}
