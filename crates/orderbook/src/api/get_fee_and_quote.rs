use crate::api::post_quote::OrderQuoteErrorWrapper;
use anyhow::Result;
use chrono::{DateTime, Utc};
use ethcontract::{H160, U256};
use model::{
    quote::{OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide, SellAmount},
    u256_decimal,
};
use serde::{Deserialize, Serialize};
use shared::{
    api::{convert_json_response, ApiReply},
    order_quoting::QuoteHandler,
};
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fee {
    #[serde(with = "u256_decimal")]
    amount: U256,
    expiration_date: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SellQuery {
    sell_token: H160,
    buy_token: H160,
    // The total amount to be sold from which the fee will be deducted.
    #[serde(with = "u256_decimal")]
    sell_amount_before_fee: U256,
}

impl From<SellQuery> for OrderQuoteRequest {
    fn from(query: SellQuery) -> Self {
        let side = OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: query.sell_amount_before_fee,
            },
        };
        OrderQuoteRequest::new(query.sell_token, query.buy_token, side)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SellResponse {
    // The fee that is deducted from sell_amount_before_fee. The sell amount that is traded is
    // sell_amount_before_fee - fee_in_sell_token.
    fee: Fee,
    // The expected buy amount for the traded sell amount.
    #[serde(with = "u256_decimal")]
    buy_amount_after_fee: U256,
}

impl From<OrderQuoteResponse> for SellResponse {
    fn from(response: OrderQuoteResponse) -> Self {
        Self {
            fee: Fee {
                amount: response.quote.fee_amount,
                expiration_date: response.expiration,
            },
            buy_amount_after_fee: response.quote.buy_amount,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuyQuery {
    sell_token: H160,
    buy_token: H160,
    // The total amount to be bought.
    #[serde(with = "u256_decimal")]
    buy_amount_after_fee: U256,
}

impl From<BuyQuery> for OrderQuoteRequest {
    fn from(query: BuyQuery) -> Self {
        let side = OrderQuoteSide::Buy {
            buy_amount_after_fee: query.buy_amount_after_fee,
        };
        OrderQuoteRequest::new(query.sell_token, query.buy_token, side)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuyResponse {
    // The fee that is deducted from sell_amount_before_fee. The sell amount that is traded is
    // sell_amount_before_fee - fee_in_sell_token.
    fee: Fee,
    #[serde(with = "u256_decimal")]
    sell_amount_before_fee: U256,
}

impl From<OrderQuoteResponse> for BuyResponse {
    fn from(response: OrderQuoteResponse) -> Self {
        Self {
            fee: Fee {
                amount: response.quote.fee_amount,
                expiration_date: response.expiration,
            },
            sell_amount_before_fee: response.quote.sell_amount,
        }
    }
}

fn sell_request() -> impl Filter<Extract = (SellQuery,), Error = Rejection> + Clone {
    warp::path!("feeAndQuote" / "sell")
        .and(warp::get())
        .and(warp::query::<SellQuery>())
}

fn buy_request() -> impl Filter<Extract = (BuyQuery,), Error = Rejection> + Clone {
    warp::path!("feeAndQuote" / "buy")
        .and(warp::get())
        .and(warp::query::<BuyQuery>())
}

pub fn get_fee_and_quote_sell(
    quotes: Arc<QuoteHandler>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    sell_request().and_then(move |query: SellQuery| {
        let quotes = quotes.clone();
        async move {
            Result::<_, Infallible>::Ok(convert_json_response(
                quotes
                    .calculate_quote(&query.into())
                    .await
                    .map(SellResponse::from)
                    .map_err(OrderQuoteErrorWrapper),
            ))
        }
    })
}

pub fn get_fee_and_quote_buy(
    quotes: Arc<QuoteHandler>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    buy_request().and_then(move |query: BuyQuery| {
        let quotes = quotes.clone();
        async move {
            Result::<_, Infallible>::Ok(convert_json_response(
                quotes
                    .calculate_quote(&query.into())
                    .await
                    .map(BuyResponse::from)
                    .map_err(OrderQuoteErrorWrapper),
            ))
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
    fn sell_query() {
        let path= "/feeAndQuote/sell?sellToken=0xdac17f958d2ee523a2206206994597c13d831ec7&buyToken=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&sellAmountBeforeFee=1000000";
        let request = request().path(path).method("GET");
        let result = request
            .filter(&sell_request())
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(
            result.sell_token,
            H160(hex!("dac17f958d2ee523a2206206994597c13d831ec7"))
        );
        assert_eq!(
            result.buy_token,
            H160(hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"))
        );
        assert_eq!(result.sell_amount_before_fee, 1000000.into());
    }

    #[test]
    fn buy_query() {
        let path= "/feeAndQuote/buy?sellToken=0xdac17f958d2ee523a2206206994597c13d831ec7&buyToken=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&buyAmountAfterFee=1000000";
        let request = request().path(path).method("GET");
        let result = request
            .filter(&buy_request())
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(
            result.sell_token,
            H160(hex!("dac17f958d2ee523a2206206994597c13d831ec7"))
        );
        assert_eq!(
            result.buy_token,
            H160(hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"))
        );
        assert_eq!(result.buy_amount_after_fee, 1000000.into());
    }
}
