use crate::fee::MinFeeCalculating;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use ethcontract::{H160, U256};
use model::h160_hexadecimal;
use model::{order::OrderKind, u256_decimal};
use serde::{Deserialize, Serialize};
use shared::price_estimate;
use shared::price_estimate::{PriceEstimating, PriceEstimationError};
use std::convert::Infallible;
use std::sync::Arc;
use warp::{hyper::StatusCode, reply, Filter, Rejection, Reply};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Fee {
    #[serde(with = "u256_decimal")]
    amount: U256,
    expiration_date: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SellQuery {
    #[serde(with = "h160_hexadecimal")]
    sell_token: H160,
    #[serde(with = "h160_hexadecimal")]
    buy_token: H160,
    // The total amount to be sold from which the fee will be deducted.
    #[serde(with = "u256_decimal")]
    sell_amount_before_fee: U256,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuyQuery {
    #[serde(with = "h160_hexadecimal")]
    sell_token: H160,
    #[serde(with = "h160_hexadecimal")]
    buy_token: H160,
    // The total amount to be bought.
    #[serde(with = "u256_decimal")]
    buy_amount_after_fee: U256,
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

#[derive(Debug)]
enum Error {
    NoLiquidity,
    UnsupportedToken(H160),
    AmountIsZero,
    SellAmountDoesNotCoverFee,
    Other(anyhow::Error),
}

impl From<PriceEstimationError> for Error {
    fn from(other: PriceEstimationError) -> Self {
        match other {
            PriceEstimationError::NoLiquidity => Error::NoLiquidity,
            PriceEstimationError::UnsupportedToken(token) => Error::UnsupportedToken(token),
            PriceEstimationError::Other(error) => Error::Other(error),
        }
    }
}

async fn calculate_sell(
    fee_calculator: Arc<dyn MinFeeCalculating>,
    price_estimator: Arc<dyn PriceEstimating>,
    query: SellQuery,
) -> Result<SellResponse, Error> {
    if query.sell_amount_before_fee.is_zero() {
        return Err(Error::AmountIsZero);
    }

    // TODO: would be nice to use true sell amount after the fee but that is more complicated.
    let (fee, expiration_date) = fee_calculator
        .min_fee(
            query.sell_token,
            Some(query.buy_token),
            Some(query.sell_amount_before_fee),
            Some(OrderKind::Sell),
        )
        .await?;
    let sell_amount_after_fee = query
        .sell_amount_before_fee
        .checked_sub(fee)
        .ok_or(Error::SellAmountDoesNotCoverFee)?
        .max(U256::one());

    let estimate = price_estimator
        .estimate(&price_estimate::Query {
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            in_amount: sell_amount_after_fee,
            kind: OrderKind::Sell,
        })
        .await?;

    Ok(SellResponse {
        fee: Fee {
            expiration_date,
            amount: fee,
        },
        buy_amount_after_fee: estimate.out_amount,
    })
}

async fn calculate_buy(
    fee_calculator: Arc<dyn MinFeeCalculating>,
    price_estimator: Arc<dyn PriceEstimating>,
    query: BuyQuery,
) -> Result<BuyResponse, Error> {
    if query.buy_amount_after_fee.is_zero() {
        return Err(Error::AmountIsZero);
    }

    let (fee, expiration_date) = fee_calculator
        .min_fee(
            query.sell_token,
            Some(query.buy_token),
            Some(query.buy_amount_after_fee),
            Some(OrderKind::Buy),
        )
        .await?;

    let estimate = price_estimator
        .estimate(&price_estimate::Query {
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            in_amount: query.buy_amount_after_fee,
            kind: OrderKind::Buy,
        })
        .await?;
    let sell_amount_before_fee = estimate
        .out_amount
        .checked_add(fee)
        .ok_or_else(|| Error::Other(anyhow!("overflow in sell_amount_before_fee")))?;

    Ok(BuyResponse {
        fee: Fee {
            expiration_date,
            amount: fee,
        },
        sell_amount_before_fee,
    })
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

fn response<T: Serialize>(result: Result<T, Error>) -> impl Reply {
    match result {
        Ok(response) => reply::with_status(reply::json(&response), StatusCode::OK),
        Err(Error::NoLiquidity) => reply::with_status(
            super::error("NoLiquidity", "not enough liquidity"),
            StatusCode::NOT_FOUND,
        ),
        Err(Error::UnsupportedToken(token)) => reply::with_status(
            super::error("UnsupportedToken", format!("Token address {:?}", token)),
            StatusCode::BAD_REQUEST,
        ),
        Err(Error::AmountIsZero) => reply::with_status(
            super::error(
                "AmountIsZero",
                "The input amount must be greater than zero.".to_string(),
            ),
            StatusCode::BAD_REQUEST,
        ),
        Err(Error::SellAmountDoesNotCoverFee) => reply::with_status(
            super::error(
                "SellAmountDoesNotCoverFee",
                "The sell amount for the sell order is lower than the fee.".to_string(),
            ),
            StatusCode::BAD_REQUEST,
        ),
        Err(Error::Other(err)) => {
            tracing::error!(?err, "get_fee_and_price error");
            reply::with_status(super::internal_error(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub fn get_fee_and_quote_sell(
    fee_calculator: Arc<dyn MinFeeCalculating>,
    price_estimator: Arc<dyn PriceEstimating>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    sell_request().and_then(move |query| {
        let fee_calculator = fee_calculator.clone();
        let price_estimator = price_estimator.clone();
        async move {
            Result::<_, Infallible>::Ok(response(
                calculate_sell(fee_calculator, price_estimator, query).await,
            ))
        }
    })
}

pub fn get_fee_and_quote_buy(
    fee_calculator: Arc<dyn MinFeeCalculating>,
    price_estimator: Arc<dyn PriceEstimating>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    buy_request().and_then(move |query| {
        let fee_calculator = fee_calculator.clone();
        let price_estimator = price_estimator.clone();
        async move {
            Result::<_, Infallible>::Ok(response(
                calculate_buy(fee_calculator, price_estimator, query).await,
            ))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fee::MockMinFeeCalculating;
    use futures::FutureExt;
    use hex_literal::hex;
    use shared::price_estimate::mocks::FakePriceEstimator;
    use warp::test::request;

    #[test]
    fn calculate_sell_() {
        let mut fee_calculator = MockMinFeeCalculating::new();
        fee_calculator
            .expect_min_fee()
            .returning(|_, _, _, _| Ok((U256::from(3), Utc::now())));
        let price_estimator = FakePriceEstimator(price_estimate::Estimate {
            out_amount: 14.into(),
            gas: 1000.into(),
        });
        let result = calculate_sell(
            Arc::new(fee_calculator),
            Arc::new(price_estimator),
            SellQuery {
                sell_token: H160::from_low_u64_ne(0),
                buy_token: H160::from_low_u64_ne(1),
                sell_amount_before_fee: 10.into(),
            },
        )
        .now_or_never()
        .unwrap()
        .unwrap();
        assert_eq!(result.fee.amount, 3.into());
        // After the deducting the fee 10 - 3 = 7 units of sell token are being sold.
        assert_eq!(result.buy_amount_after_fee, 14.into());
    }

    #[test]
    fn calculate_buy_() {
        let mut fee_calculator = MockMinFeeCalculating::new();
        fee_calculator
            .expect_min_fee()
            .returning(|_, _, _, _| Ok((U256::from(3), Utc::now())));
        let price_estimator = FakePriceEstimator(price_estimate::Estimate {
            out_amount: 20.into(),
            gas: 1000.into(),
        });
        let result = calculate_buy(
            Arc::new(fee_calculator),
            Arc::new(price_estimator),
            BuyQuery {
                sell_token: H160::from_low_u64_ne(0),
                buy_token: H160::from_low_u64_ne(1),
                buy_amount_after_fee: 10.into(),
            },
        )
        .now_or_never()
        .unwrap()
        .unwrap();
        assert_eq!(result.fee.amount, 3.into());
        // To buy 10 units of buy_token the fee in sell_token must be at least 3 and at least 20
        // units of sell_token must be sold.
        assert_eq!(result.sell_amount_before_fee, 23.into());
    }

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
