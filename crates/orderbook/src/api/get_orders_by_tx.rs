use crate::orderbook::Orderbook;
use anyhow::Result;
use ethcontract::H256;
use shared::api::{convert_json_response, ApiReply};
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

pub fn get_orders_by_tx_request() -> impl Filter<Extract = (H256,), Error = Rejection> + Clone {
    warp::path!("v1" / "transactions" / H256 / "orders").and(warp::get())
}

pub fn get_orders_by_tx(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_orders_by_tx_request().and_then(move |hash: H256| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_orders_for_tx(&hash).await;
            Result::<_, Infallible>::Ok(convert_json_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn request_ok() {
        let hash_str = "0x0191dbb560e936bd3320d5a505c9c05580a0ebb7e12fe117551ac26e484f295e";
        let result = warp::test::request()
            .path(&format!("/v1/transactions/{:}/orders", hash_str))
            .method("GET")
            .filter(&get_orders_by_tx_request())
            .await
            .unwrap();
        assert_eq!(result.0, H256::from_str(hash_str).unwrap().0);
    }
}
