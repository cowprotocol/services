use crate::{
    api::convert_json_response,
    {database::orders::OrderFilter, orderbook::Orderbook},
};
use anyhow::{Context, Result};
use model::time::now_in_epoch_seconds;
use primitive_types::H160;
use serde::Deserialize;
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, Filter, Rejection};

// The default values create a filter that only includes valid orders.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    #[serde(default = "now_in_epoch_seconds")]
    min_valid_to: u32,
    owner: Option<H160>,
    sell_token: Option<H160>,
    buy_token: Option<H160>,
    #[serde(default)]
    include_fully_executed: bool,
    #[serde(default)]
    include_invalidated: bool,
    #[serde(default)]
    include_insufficient_balance: bool,
    #[serde(default)]
    include_unsupported_tokens: bool,
    #[serde(default)]
    include_presignature_pending: bool,
}

impl Query {
    fn order_filter(&self) -> Result<OrderFilter, &'static str> {
        if self.owner.is_none() && self.sell_token.is_none() && self.buy_token.is_none() {
            return Err("need to set at least one of owner, sell_token, buy_token");
        }
        Ok(OrderFilter {
            min_valid_to: self.min_valid_to,
            owner: self.owner,
            sell_token: self.sell_token,
            buy_token: self.buy_token,
            exclude_fully_executed: !self.include_fully_executed,
            exclude_invalidated: !self.include_invalidated,
            exclude_insufficient_balance: !self.include_insufficient_balance,
            exclude_unsupported_tokens: !self.include_unsupported_tokens,
            exclude_presignature_pending: !self.include_presignature_pending,
            uid: None,
        })
    }
}

pub fn get_orders_request(
) -> impl Filter<Extract = (Result<OrderFilter, &'static str>,), Error = Rejection> + Clone {
    warp::path!("orders")
        .and(warp::get())
        .and(warp::query::<Query>())
        .map(|query: Query| query.order_filter())
}

pub fn get_orders(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    get_orders_request().and_then(move |order_filter| {
        let orderbook = orderbook.clone();
        async move {
            let order_filter = match order_filter {
                Ok(order_filter) => order_filter,
                Err(err) => {
                    let err = super::error("InvalidOrderFilter", err);
                    return Ok(warp::reply::with_status(err, StatusCode::BAD_REQUEST));
                }
            };
            let result = orderbook
                .get_orders(&order_filter)
                .await
                .context("get_orders");
            Result::<_, Infallible>::Ok(convert_json_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::addr;
    use warp::test::{request, RequestBuilder};

    #[tokio::test]
    async fn get_orders_request_ok() {
        let order_filter = |request: RequestBuilder| async move {
            let filter = get_orders_request();
            request.method("GET").filter(&filter).await
        };

        let owner = addr!("0000000000000000000000000000000000000001");
        let sell = addr!("0000000000000000000000000000000000000002");
        let buy = addr!("0000000000000000000000000000000000000003");
        let path = format!(
            "/orders\
                 ?owner=0x{:x}\
                 &sellToken=0x{:x}\
                 &buyToken=0x{:x}\
                 &minValidTo=2\
                 &includeFullyExecuted=true\
                 &includeInvalidated=true\
                 &includeInsufficientBalance=true\
                 &includePresignaturePending=true",
            owner, sell, buy
        );
        let request = request().path(path.as_str());
        let result = order_filter(request).await.unwrap().unwrap();
        assert_eq!(result.owner, Some(owner));
        assert_eq!(result.buy_token, Some(buy));
        assert_eq!(result.sell_token, Some(sell));
        assert_eq!(result.min_valid_to, 2);
        assert!(!result.exclude_fully_executed);
        assert!(!result.exclude_invalidated);
        assert!(!result.exclude_insufficient_balance);
        assert!(!result.exclude_presignature_pending);
    }

    #[test]
    fn cannot_create_too_generic_filter() {
        let query = Query {
            owner: None,
            sell_token: None,
            buy_token: None,
            ..Default::default()
        };
        assert!(query.order_filter().is_err());
    }
}
