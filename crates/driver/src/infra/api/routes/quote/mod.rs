use crate::infra::api::State;

mod dto;

pub(in crate::infra::api) fn quote(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/quote", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    order: axum::extract::Json<dto::Order>,
) -> axum::response::Json<dto::Quote> {
    // TODO Report errors instead of unwrapping
    let order = order.0.into_domain();
    let quote = order
        .quote(state.solver(), state.quote_config())
        .await
        .unwrap();
    axum::response::Json(dto::Quote::from_domain(&quote))
}
