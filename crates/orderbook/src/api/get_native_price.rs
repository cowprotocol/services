use {
    crate::api::{ApiReply, IntoWarpReply},
    alloy::primitives::Address,
    anyhow::Result,
    model::quote::NativeTokenPrice,
    shared::price_estimation::native::NativePriceEstimating,
    std::{convert::Infallible, sync::Arc, time::Duration},
    warp::{Filter, Rejection, hyper::StatusCode, reply::with_status},
};

fn get_native_prices_request() -> impl Filter<Extract = (Address,), Error = Rejection> + Clone {
    warp::path!("v1" / "token" / Address / "native_price").and(warp::get())
}

pub fn get_native_price(
    estimator: Arc<dyn NativePriceEstimating>,
    quote_timeout: Duration,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_native_prices_request().and_then(move |token: Address| {
        let estimator = estimator.clone();
        async move {
            let result = estimator.estimate_native_price(token, quote_timeout).await;
            let reply = match result {
                Ok(price) => with_status(
                    warp::reply::json(&NativeTokenPrice { price }),
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
    use {super::*, alloy::primitives::address, futures::FutureExt, warp::test::request};

    #[test]
    fn native_prices_query() {
        let path = "/v1/token/0xdac17f958d2ee523a2206206994597c13d831ec7/native_price";
        let request = request().path(path).method("GET");
        let result = request
            .filter(&get_native_prices_request())
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, address!("dac17f958d2ee523a2206206994597c13d831ec7"));
    }
}
