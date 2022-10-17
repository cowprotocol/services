use crate::orderbook::Orderbook;
use anyhow::Result;
use ethcontract::H160;
use serde::Deserialize;
use shared::api::ApiReply;
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, reply::with_status, Filter, Rejection};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeTokens {
    tokens: Vec<H160>,
}

fn get_native_prices_request() -> impl Filter<Extract = (NativeTokens,), Error = Rejection> + Clone
{
    warp::path!("nativePrices")
        .and(warp::get())
        .and(warp::query::<NativeTokens>())
}

pub fn get_native_prices(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_native_prices_request().and_then(move |tokens: NativeTokens| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_native_prices(&tokens.tokens).await;
            let reply = match result {
                Ok(estimates) => with_status(warp::reply::json(&estimates), StatusCode::OK),
                Err(_) => with_status(
                    super::error("NotFound", "There is no estimate for all tokens"),
                    StatusCode::NOT_FOUND,
                ),
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
        let path= "/nativePrices?tokens=0xdac17f958d2ee523a2206206994597c13d831ec7,0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
        let request = request().path(path).method("GET");
        let result = request
            .filter(&get_native_prices_request())
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(
            result.tokens,
            vec![
                H160(hex!("dac17f958d2ee523a2206206994597c13d831ec7")),
                H160(hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"))
            ]
        );
    }
}
