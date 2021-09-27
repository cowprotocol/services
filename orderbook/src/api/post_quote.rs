use crate::api;
use anyhow::{anyhow, Result};
use ethcontract::{H160, U256};
use model::{
    app_id::AppId,
    order::{BuyTokenDestination, OrderKind, SellTokenSource},
    u256_decimal,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use warp::{hyper::StatusCode, reply, Filter, Rejection, Reply};

/// The order parameters to quote a price and fee for.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct OrderQuoteRequest {
    from: H160,
    sell_token: H160,
    buy_token: H160,
    receiver: Option<H160>,
    #[serde(flatten)]
    side: OrderQuoteSide,
    valid_to: u32,
    app_data: AppId,
    partially_fillable: bool,
    #[serde(default)]
    sell_token_balance: SellTokenSource,
    #[serde(default)]
    buy_token_balance: BuyTokenDestination,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum OrderQuoteSide {
    #[serde(rename_all = "camelCase")]
    Sell {
        #[serde(flatten)]
        sell_amount: SellAmount,
    },
    #[serde(rename_all = "camelCase")]
    Buy {
        #[serde(with = "u256_decimal")]
        buy_amount_after_fee: U256,
    },
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum SellAmount {
    BeforeFee {
        #[serde(rename = "sellAmountBeforeFee", with = "u256_decimal")]
        value: U256,
    },
    AfterFee {
        #[serde(rename = "sellAmountAfterFee", with = "u256_decimal")]
        value: U256,
    },
}

/// The quoted order by the service.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderQuote {
    from: H160,
    sell_token: H160,
    buy_token: H160,
    receiver: Option<H160>,
    #[serde(with = "u256_decimal")]
    sell_amount: U256,
    #[serde(with = "u256_decimal")]
    buy_amount: U256,
    valid_to: u32,
    app_data: AppId,
    #[serde(with = "u256_decimal")]
    fee_amount: U256,
    kind: OrderKind,
    partially_fillable: bool,
    sell_token_balance: SellTokenSource,
    buy_token_balance: BuyTokenDestination,
}

fn post_quote_request() -> impl Filter<Extract = (OrderQuoteRequest,), Error = Rejection> + Clone {
    warp::path!("quote")
        .and(warp::post())
        .and(api::extract_payload())
}

fn post_order_response(result: Result<OrderQuote>) -> impl Reply {
    match result {
        Ok(response) => reply::with_status(reply::json(&response), StatusCode::OK),
        Err(err) => reply::with_status(
            super::error("InternalServerError", err.to_string()),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

pub fn post_quote() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    post_quote_request().and_then(move |request| async move {
        tracing::warn!("unimplemented request {:#?}", request);
        Result::<_, Infallible>::Ok(post_order_response(Err(anyhow!("not yet implemented"))))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_sell_after_fees_quote_request() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "kind": "sell",
                "sellAmountAfterFee": "1337",
                "validTo": 0x12345678,
                "appData": "0x9090909090909090909090909090909090909090909090909090909090909090",
                "partiallyFillable": false,
                "buyTokenBalance": "internal",
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: H160([0x01; 20]),
                sell_token: H160([0x02; 20]),
                buy_token: H160([0x03; 20]),
                receiver: None,
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee { value: 1337.into() },
                },
                valid_to: 0x12345678,
                app_data: AppId([0x90; 32]),
                partially_fillable: false,
                sell_token_balance: SellTokenSource::Erc20,
                buy_token_balance: BuyTokenDestination::Internal,
            }
        );
    }

    #[test]
    fn deserializes_sell_before_fees_quote_request() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "kind": "sell",
                "sellAmountBeforeFee": "1337",
                "validTo": 0x12345678,
                "appData": "0x9090909090909090909090909090909090909090909090909090909090909090",
                "partiallyFillable": false,
                "sellTokenBalance": "external",
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: H160([0x01; 20]),
                sell_token: H160([0x02; 20]),
                buy_token: H160([0x03; 20]),
                receiver: None,
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::BeforeFee { value: 1337.into() },
                },
                valid_to: 0x12345678,
                app_data: AppId([0x90; 32]),
                partially_fillable: false,
                sell_token_balance: SellTokenSource::External,
                buy_token_balance: BuyTokenDestination::Erc20,
            }
        );
    }

    #[test]
    fn deserializes_buy_quote_request() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "receiver": "0x0404040404040404040404040404040404040404",
                "kind": "buy",
                "buyAmountAfterFee": "1337",
                "validTo": 0x12345678,
                "appData": "0x9090909090909090909090909090909090909090909090909090909090909090",
                "partiallyFillable": false,
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: H160([0x01; 20]),
                sell_token: H160([0x02; 20]),
                buy_token: H160([0x03; 20]),
                receiver: Some(H160([0x04; 20])),
                side: OrderQuoteSide::Buy {
                    buy_amount_after_fee: U256::from(1337),
                },
                valid_to: 0x12345678,
                app_data: AppId([0x90; 32]),
                partially_fillable: false,
                sell_token_balance: SellTokenSource::Erc20,
                buy_token_balance: BuyTokenDestination::Erc20,
            }
        );
    }
}
