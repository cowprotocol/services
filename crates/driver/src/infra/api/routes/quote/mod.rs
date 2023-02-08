use crate::infra::api::{Error, State};

mod dto;

pub use dto::OrderError;

pub(in crate::infra::api) fn quote(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/quote", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    order: axum::Json<dto::Order>,
) -> Result<axum::Json<dto::Quote>, axum::Json<Error>> {
    let order = order.0.into_domain()?;
    let quote = order
        .quote(state.eth(), state.solver(), state.liquidity(), state.now())
        .await?;
    Ok(axum::response::Json(dto::Quote::from_domain(&quote)))
}
