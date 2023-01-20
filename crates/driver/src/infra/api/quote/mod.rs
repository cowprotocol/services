mod dto;

pub(super) fn route(router: axum::Router<super::State>) -> axum::Router<super::State> {
    router.route("/quote", axum::routing::post(quote))
}

async fn quote(
    state: axum::extract::State<super::State>,
    order: axum::extract::Json<dto::Order>,
) -> axum::response::Json<dto::Quote> {
    // TODO Report errors instead of unwrapping
    let order = order.0.into_domain(state.now()).unwrap();
    let liquidity = state.liquidity().for_quote(&order).await.unwrap();
    let quote = order
        .quote(state.eth(), state.solver(), &liquidity, state.now())
        .await
        .unwrap();
    axum::response::Json(dto::Quote::from_domain(&quote))
}
