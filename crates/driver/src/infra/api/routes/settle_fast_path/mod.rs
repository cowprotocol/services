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

pub(in crate::infra::api) fn settle_fast_path(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle_fast_path", axum::routing::post(route))
}

/// Settles a quote solution the driver cached at quote time, outside of any
/// competition. The cached solution's order is synthetic and unsigned, so it
/// is first re-encoded against the real signed order; that yields a regular
/// queued solution, which is then settled like any other.
async fn route(
    state: axum::extract::State<State>,
    LoggingJson(req): LoggingJson<dto::SettleFastPathRequest>,
) -> Result<(), (axum::http::StatusCode, axum::Json<Error>)> {
    let auction_id =
        auction::Id::try_from(req.auction_id).map_err(api::routes::AuctionError::from)?;
    let solver = state.solver().name().to_string();
    let quote_id = req.quote_id;

    async move {
        observe::settling();
        let order = req.order.into_domain(None);
        let limit_prices = solution::LimitPrices {
            sell: req.limit_prices.sell,
            buy: req.limit_prices.buy,
        };
        let cached = state
            .quote_cache()
            .take(&quote::Id(quote_id))
            .await
            .ok_or(crate::domain::competition::Error::SolutionNotAvailable)?;
        let solution_id = state
            .competition()
            .reencode_quote_solution(auction_id, cached, order, limit_prices)
            .await?;
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
    .instrument(tracing::info_span!(
        "/settle_fast_path",
        solver,
        %auction_id,
        quote_id
    ))
    .await
}
