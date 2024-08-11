use {
    super::with_status,
    axum::{http::StatusCode, routing::MethodRouter},
    ethcontract::H160,
    serde::Serialize,
    shared::api::{ApiReply, IntoApiReply},
};

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::get(handler))
}

const ENDPOINT: &str = "/api/v1/token/:token/native_price";
async fn handler(
    state: axum::extract::State<super::State>,
    token: axum::extract::Path<H160>,
) -> ApiReply {
    let result = state
        .native_price_estimator
        .estimate_native_price(token.0)
        .await;
    match result {
        Ok(price) => with_status(
            serde_json::to_value(PriceResponse::from(price)).unwrap(),
            StatusCode::OK,
        ),
        Err(err) => err.into_api_reply(),
    }
}

#[derive(Serialize)]
struct PriceResponse {
    price: f64,
}

impl From<f64> for PriceResponse {
    fn from(price: f64) -> Self {
        Self { price }
    }
}

// #[cfg(test)]
// mod tests {
//     use {super::*, futures::FutureExt, hex_literal::hex,
// warp::test::request};

//     #[test]
//     fn native_prices_query() {
//         let path =
// "/v1/token/0xdac17f958d2ee523a2206206994597c13d831ec7/native_price";
//         let request = request().path(path).method("GET");
//         let result = request
//             .filter(&get_native_prices_request())
//             .now_or_never()
//             .unwrap()
//             .unwrap();
//         assert_eq!(
//             result,
//             H160(hex!("dac17f958d2ee523a2206206994597c13d831ec7"))
//         );
//     }
// }
