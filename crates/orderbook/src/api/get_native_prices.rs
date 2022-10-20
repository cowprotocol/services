use anyhow::Result;
use ethcontract::H160;
use futures::StreamExt;
use serde::Serialize;
use shared::{
    api::{ApiReply, IntoWarpReply},
    price_estimation::native::NativePriceEstimating,
};
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, reply::with_status, Filter, Rejection};

#[derive(Serialize)]
struct PriceResponse {
    price: f64,
}

impl From<f64> for PriceResponse {
    fn from(price: f64) -> Self {
        Self { price }
    }
}

fn get_native_prices_request() -> impl Filter<Extract = (H160,), Error = Rejection> + Clone {
    warp::path!("token" / H160 / "native_price").and(warp::get())
}

pub fn get_native_prices(
    estimator: Arc<dyn NativePriceEstimating>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_native_prices_request().and_then(move |token: H160| {
        let estimator = estimator.clone();
        async move {
            let result = estimator
                .estimate_native_prices(std::slice::from_ref(&token))
                .next()
                .await
                .unwrap()
                .1;
            let reply = match result {
                Ok(price) => with_status(
                    warp::reply::json(&PriceResponse::from(price)),
                    StatusCode::OK,
                ),
                Err(err) => err.into_warp_reply(),
            };
            Result::<_, Infallible>::Ok(reply)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use hex_literal::hex;
    use warp::test::request;

    #[test]
    fn native_prices_query() {
        let path = "/token/0xdac17f958d2ee523a2206206994597c13d831ec7/native_price";
        let request = request().path(path).method("GET");
        let result = request
            .filter(&get_native_prices_request())
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(
            result,
            H160(hex!("dac17f958d2ee523a2206206994597c13d831ec7"))
        );
    }
}
