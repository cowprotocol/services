use {
    super::with_status,
    axum::routing::MethodRouter,
    ethcontract::H256,
    reqwest::StatusCode,
    shared::api::{internal_error_reply, ApiReply},
};

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::get(handler))
}

const ENDPOINT: &str = "/api/v1/transactions/:tx_hash/orders";
async fn handler(
    state: axum::extract::State<super::State>,
    tx_hash: axum::extract::Path<H256>,
) -> ApiReply {
    let result = state.orderbook.get_orders_for_tx(&tx_hash.0).await;
    match result {
        Ok(response) => with_status(serde_json::to_value(&response).unwrap(), StatusCode::OK),
        Err(err) => {
            tracing::error!(?err, "get_orders_by_tx");
            internal_error_reply()
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use {super::*, std::str::FromStr};

//     #[tokio::test]
//     async fn request_ok() {
//         let hash_str =
// "0x0191dbb560e936bd3320d5a505c9c05580a0ebb7e12fe117551ac26e484f295e";
//         let result = warp::test::request()
//             .path(&format!("/v1/transactions/{hash_str}/orders"))
//             .method("GET")
//             .filter(&get_orders_by_tx_request())
//             .await
//             .unwrap();
//         assert_eq!(result.0, H256::from_str(hash_str).unwrap().0);
//     }
// }
