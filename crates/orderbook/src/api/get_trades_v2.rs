use {
    crate::{
        api::{ApiReply, error},
        database::{
            Postgres,
            trades::{PaginatedTradeFilter, TradeRetrievingPaginated},
        },
    },
    alloy::primitives::Address,
    anyhow::{Context, Result},
    model::order::OrderUid,
    serde::Deserialize,
    std::convert::Infallible,
    warp::{Filter, Rejection, hyper::StatusCode, reply::with_status},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    pub order_uid: Option<OrderUid>,
    pub owner: Option<Address>,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

const DEFAULT_OFFSET: u64 = 0;
const DEFAULT_LIMIT: u64 = 10;
const MIN_LIMIT: u64 = 1;
const MAX_LIMIT: u64 = 1000;

#[derive(Debug, Eq, PartialEq)]
enum TradeFilterError {
    InvalidFilter(String),
    InvalidLimit(u64, u64),
}

impl Query {
    fn trade_filter(&self, offset: u64, limit: u64) -> PaginatedTradeFilter {
        PaginatedTradeFilter {
            order_uid: self.order_uid,
            owner: self.owner,
            offset,
            limit,
        }
    }

    fn validate(&self) -> Result<PaginatedTradeFilter, TradeFilterError> {
        match (self.order_uid.as_ref(), self.owner.as_ref()) {
            (Some(_), None) | (None, Some(_)) => {
                let offset = self.offset.unwrap_or(DEFAULT_OFFSET);
                let limit = self.limit.unwrap_or(DEFAULT_LIMIT);

                if !(MIN_LIMIT..=MAX_LIMIT).contains(&limit) {
                    return Err(TradeFilterError::InvalidLimit(MIN_LIMIT, MAX_LIMIT));
                }

                Ok(self.trade_filter(offset, limit))
            }
            _ => Err(TradeFilterError::InvalidFilter(
                "Must specify exactly one of owner or orderUid.".to_owned(),
            )),
        }
    }
}

fn get_trades_request()
-> impl Filter<Extract = (Result<PaginatedTradeFilter, TradeFilterError>,), Error = Rejection> + Clone
{
    warp::path!("v2" / "trades")
        .and(warp::get())
        .and(warp::query::<Query>())
        .map(|query: Query| query.validate())
}

pub fn get_trades(db: Postgres) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_trades_request().and_then(move |request_result| {
        let database = db.clone();
        async move {
            Result::<_, Infallible>::Ok(match request_result {
                Ok(trade_filter) => {
                    let result = database
                        .trades_paginated(&trade_filter)
                        .await
                        .context("get_trades_v2");
                    match result {
                        Ok(reply) => with_status(warp::reply::json(&reply), StatusCode::OK),
                        Err(err) => {
                            tracing::error!(?err, "get_trades_v2");
                            crate::api::internal_error_reply()
                        }
                    }
                }
                Err(TradeFilterError::InvalidFilter(msg)) => {
                    let err = error("InvalidTradeFilter", msg);
                    with_status(err, StatusCode::BAD_REQUEST)
                }
                Err(TradeFilterError::InvalidLimit(min, max)) => {
                    let err = error(
                        "InvalidLimit",
                        format!("limit must be between {min} and {max}"),
                    );
                    with_status(err, StatusCode::BAD_REQUEST)
                }
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        warp::test::{RequestBuilder, request},
    };

    #[tokio::test]
    async fn get_trades_request_ok() {
        let trade_filter = |request: RequestBuilder| async move {
            let filter = get_trades_request();
            request.method("GET").filter(&filter).await
        };

        let owner = Address::with_last_byte(1);
        let owner_path = format!("/v2/trades?owner=0x{owner:x}");
        let result = trade_filter(request().path(owner_path.as_str()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.owner, Some(owner));
        assert_eq!(result.order_uid, None);
        assert_eq!(result.offset, DEFAULT_OFFSET);
        assert_eq!(result.limit, DEFAULT_LIMIT);

        let uid = OrderUid([1u8; 56]);
        let order_uid_path = format!("/v2/trades?orderUid={uid}");
        let result = trade_filter(request().path(order_uid_path.as_str()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.owner, None);
        assert_eq!(result.order_uid, Some(uid));
        assert_eq!(result.offset, DEFAULT_OFFSET);
        assert_eq!(result.limit, DEFAULT_LIMIT);

        // Test with custom offset and limit
        let owner_path = format!("/v2/trades?owner=0x{owner:x}&offset=10&limit=50");
        let result = trade_filter(request().path(owner_path.as_str()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.owner, Some(owner));
        assert_eq!(result.offset, 10);
        assert_eq!(result.limit, 50);
    }

    #[tokio::test]
    async fn get_trades_request_err() {
        let trade_filter = |request: RequestBuilder| async move {
            let filter = get_trades_request();
            request.method("GET").filter(&filter).await
        };

        let owner = Address::with_last_byte(1);
        let uid = OrderUid([1u8; 56]);
        let path = format!("/v2/trades?owner=0x{owner:x}&orderUid={uid}");

        let result = trade_filter(request().path(path.as_str())).await.unwrap();
        assert!(result.is_err());

        let path = "/v2/trades";
        let result = trade_filter(request().path(path)).await.unwrap();
        assert!(result.is_err());

        // Test limit validation
        let path = format!("/v2/trades?owner=0x{owner:x}&limit=0");
        let result = trade_filter(request().path(path.as_str())).await.unwrap();
        assert!(matches!(
            result,
            Err(TradeFilterError::InvalidLimit(MIN_LIMIT, MAX_LIMIT))
        ));

        let path = format!("/v2/trades?owner=0x{owner:x}&limit=1001");
        let result = trade_filter(request().path(path.as_str())).await.unwrap();
        assert!(matches!(
            result,
            Err(TradeFilterError::InvalidLimit(MIN_LIMIT, MAX_LIMIT))
        ));
    }
}
