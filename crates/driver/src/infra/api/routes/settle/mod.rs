mod dto;

use {
    crate::{
        domain::{
            competition::{self, auction, solution},
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
        auction::Id::try_from(req.auction_id).map_err(api::routes::AuctionError::from)?;
    let solver = state.solver().name().to_string();

    async move {
        observe::settling();
        let solution_id = match req.fast_path {
            // A fast-path settlement references the quote cached by `/quote`;
            // re-encoding it against the real order yields the solution to
            // settle.
            Some(fast_path) => {
                let order = fast_path.order.into_domain(None);
                let limit_prices = solution::LimitPrices {
                    sell: fast_path.limit_prices.sell,
                    buy: fast_path.limit_prices.buy,
                };
                state
                    .competition()
                    .reencode_quote_solution(
                        auction_id,
                        quote::Id(fast_path.quote_id),
                        order,
                        limit_prices,
                    )
                    .await?
            }
            // The autopilot always sends a solution id when it is not
            // settling a cached quote.
            None => req
                .solution_id
                .ok_or(competition::Error::MalformedRequest)?,
        };
        let result = state
            .competition()
            .settle(
                auction_id,
                solution_id,
                req.submission_deadline_latest_block.into(),
            )
            .await;
        result.map(|_| ()).map_err(Into::into)
    }
    .instrument(tracing::info_span!("/settle", solver, %auction_id))
    .await
}
