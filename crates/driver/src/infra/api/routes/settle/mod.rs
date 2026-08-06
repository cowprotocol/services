mod dto;

use {
    crate::{
        domain::competition::{
            auction,
            order::app_data::{AppData, AppDataHash},
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
        if let Some(order) = req.order {
            let app_data = AppData::Hash(AppDataHash::from(order.app_data));
            let prices = req
                .prices
                .into_iter()
                .filter_map(|(token, price)| {
                    auction::Price::try_new(price.into())
                        .ok()
                        .map(|price| (token.into(), price))
                })
                .collect();
            state
                .competition()
                .reencode_quote_solution(
                    auction_id,
                    req.solution_id,
                    order.into_domain(app_data),
                    prices,
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
