use {
    axum::routing::MethodRouter,
    model::order::SignedOrderCancellations,
    shared::api::{convert_json_response, ApiReply},
};

const ENDPOINT: &str = "/api/v1/orders";

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::delete(handler))
}

async fn handler(
    state: axum::extract::State<super::State>,
    cancellations: axum::extract::Json<SignedOrderCancellations>,
) -> ApiReply {
    let result = state.orderbook.cancel_orders(cancellations.0).await;
    convert_json_response(result.map(|_| "Cancelled"))
}
