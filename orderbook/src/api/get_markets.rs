use crate::api::convert_json_response;
use anyhow::{anyhow, Result};
use ethcontract::{H160, U256};
use model::order::OrderKind;
use serde::{Deserialize, Serialize};
use shared::price_estimation::{self, PriceEstimating};
use std::{convert::Infallible, str::FromStr, sync::Arc};
use warp::{Filter, Rejection, Reply};

#[derive(Clone, Debug, PartialEq)]
struct AmountEstimateQuery {
    market: Market,
    amount: U256,
    kind: OrderKind,
}

#[derive(Deserialize, Serialize)]
struct AmountEstimateResult {
    #[serde(with = "model::u256_decimal")]
    amount: U256,
    token: H160,
}

struct TokenAmount(U256);
impl FromStr for TokenAmount {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(U256::from_dec_str(s)?))
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
struct Market {
    base_token: H160,
    quote_token: H160,
}

impl FromStr for Market {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            Err(anyhow!(
                "Market needs to be consist of two addresses separated by -"
            ))
        } else {
            Ok(Market {
                base_token: H160::from_str(parts[0])?,
                quote_token: H160::from_str(parts[1])?,
            })
        }
    }
}

fn get_amount_estimate_request(
) -> impl Filter<Extract = (AmountEstimateQuery,), Error = Rejection> + Clone {
    warp::path!("markets" / Market / OrderKind / TokenAmount)
        .and(warp::get())
        .map(|market, kind, amount: TokenAmount| AmountEstimateQuery {
            market,
            kind,
            amount: amount.0,
        })
}

pub fn get_amount_estimate(
    price_estimator: Arc<dyn PriceEstimating>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    get_amount_estimate_request().and_then(move |query: AmountEstimateQuery| {
        let price_estimator = price_estimator.clone();
        async move {
            let market = &query.market;
            let (buy_token, sell_token) = match query.kind {
                // Buy in WETH/DAI means buying ETH (selling DAI)
                OrderKind::Buy => (market.base_token, market.quote_token),
                // Sell in WETH/DAI means selling ETH (buying DAI)
                OrderKind::Sell => (market.quote_token, market.base_token),
            };
            let result = price_estimator
                .estimate(&price_estimation::Query {
                    sell_token,
                    buy_token,
                    in_amount: query.amount,
                    kind: query.kind,
                })
                .await;
            Result::<_, Infallible>::Ok(convert_json_response(result.map(|estimate| {
                AmountEstimateResult {
                    amount: estimate.out_amount,
                    token: query.market.quote_token,
                }
            })))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response_body;
    use hex_literal::hex;
    use shared::price_estimation::PriceEstimationError;
    use warp::hyper::StatusCode;
    use warp::test::request;

    #[tokio::test]
    async fn test_get_amount_estimate_request() {
        let get_query = |path| async move {
            request()
                .path(path)
                .filter(&get_amount_estimate_request())
                .await
                .unwrap()
        };

        let request = get_query("/markets/0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2-0x6b175474e89094c44da98b954eedeac495271d0f/sell/100").await;
        assert_eq!(
            request,
            AmountEstimateQuery {
                market: Market {
                    base_token: H160(hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
                    quote_token: H160(hex!("6b175474e89094c44da98b954eedeac495271d0f")),
                },
                kind: OrderKind::Sell,
                amount: 100.into()
            }
        );

        let request = get_query("/markets/0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2-0x6b175474e89094c44da98b954eedeac495271d0f/buy/100").await;
        assert_eq!(request.kind, OrderKind::Buy);
    }

    #[tokio::test]
    async fn test_get_amount_estimate_response_ok() {
        let query = AmountEstimateQuery {
            market: Market {
                base_token: H160::from_low_u64_be(1),
                quote_token: H160::from_low_u64_be(2),
            },
            amount: 100.into(),
            kind: OrderKind::Sell,
        };

        // Sell Order
        let response = convert_json_response::<_, PriceEstimationError>(Ok(AmountEstimateResult {
            amount: 2.into(),
            token: query.market.quote_token,
        }))
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let estimate: AmountEstimateResult =
            serde_json::from_slice(response_body(response).await.as_slice()).unwrap();
        assert_eq!(estimate.amount, 2.into());
        assert_eq!(estimate.token, query.market.quote_token);

        // Buy Order
        let response = convert_json_response::<_, PriceEstimationError>(Ok(AmountEstimateResult {
            amount: 2.into(),
            token: query.market.quote_token,
        }))
        .into_response();

        let estimate: AmountEstimateResult =
            serde_json::from_slice(response_body(response).await.as_slice()).unwrap();
        assert_eq!(estimate.amount, 2.into());
        assert_eq!(estimate.token, query.market.quote_token);
    }
}
