mod dto;

use {
    crate::{
        domain::competition::{
            auction,
            order::app_data::{AppData, AppDataHash},
            solution,
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
        if let Some(fast_path) = req.fast_path {
            let app_data = AppData::Hash(AppDataHash::from(fast_path.order.app_data));
            let order = fast_path.order.into_domain(app_data);
            let limit_prices = solution::LimitPrices {
                sell: fast_path.limit_prices.sell,
                buy: fast_path.limit_prices.buy,
            };
            state
                .competition()
                .reencode_quote_solution(
                    auction_id,
                    req.solution_id,
                    order,
                    limit_prices,
                    fast_path.native_prices,
                )
                .await?;
        }
        let result = state
            .competition()
            .settle(
                auction_id,
                req.solution_id,
                req.submission_deadline_latest_block.into(),
            )
            .await;
        result.map(|_| ()).map_err(Into::into)
    }
    .instrument(tracing::info_span!("/settle", solver, %auction_id))
    .await
}
