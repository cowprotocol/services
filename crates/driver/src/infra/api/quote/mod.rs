mod dto;

pub(super) fn route(router: axum::Router<super::State>) -> axum::Router<super::State> {
    router.route("/quote", axum::routing::post(quote))
}

async fn quote(
    state: axum::extract::State<super::State>,
    order: axum::extract::Json<dto::Order>,
) -> axum::response::Json<dto::Quote> {
    // TODO Report errors instead of unwrapping
    let order = order.0.into_domain();
    let quote = order.quote(state.solver(), state.now()).await.unwrap();
    axum::response::Json(dto::Quote::from_domain(&quote))
}
