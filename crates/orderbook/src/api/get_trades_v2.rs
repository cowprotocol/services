use {
    crate::{
        api::{AppState, error},
        database::trades::{PaginatedTradeFilter, TradeRetrievingPaginated},
    },
    alloy::primitives::Address,
    anyhow::Context,
    axum::{
        extract::{Query, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::order::OrderUid,
    serde::Deserialize,
    std::sync::Arc,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueryParams {
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

impl QueryParams {
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

pub async fn get_trades_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<QueryParams>,
) -> Response {
    let trade_filter = match query.validate() {
        Ok(trade_filter) => trade_filter,
        Err(TradeFilterError::InvalidFilter(msg)) => {
            let err = error("InvalidTradeFilter", msg);
            return (StatusCode::BAD_REQUEST, err).into_response();
        }
        Err(TradeFilterError::InvalidLimit(min, max)) => {
            let err = error(
                "InvalidLimit",
                format!("limit must be between {min} and {max}"),
            );
            return (StatusCode::BAD_REQUEST, err).into_response();
        }
    };

    let result = state
        .database_read
        .trades_paginated(&trade_filter)
        .await
        .context("get_trades_v2");
    match result {
        Ok(reply) => (StatusCode::OK, Json(reply)).into_response(),
        Err(err) => {
            tracing::error!(?err, "get_trades_v2");
            crate::api::internal_error_reply()
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::Address, model::order::OrderUid};

    #[test]
    fn query_validation_ok() {
        let owner = Address::with_last_byte(1);
        let query = QueryParams {
            owner: Some(owner),
            order_uid: None,
            offset: None,
            limit: None,
        };
        let result = query.validate().unwrap();
        assert_eq!(result.owner, Some(owner));
        assert_eq!(result.order_uid, None);
        assert_eq!(result.offset, DEFAULT_OFFSET);
        assert_eq!(result.limit, DEFAULT_LIMIT);

        let uid = OrderUid([1u8; 56]);
        let query = QueryParams {
            owner: None,
            order_uid: Some(uid),
            offset: None,
            limit: None,
        };
        let result = query.validate().unwrap();
        assert_eq!(result.owner, None);
        assert_eq!(result.order_uid, Some(uid));
        assert_eq!(result.offset, DEFAULT_OFFSET);
        assert_eq!(result.limit, DEFAULT_LIMIT);

        // Test with custom offset and limit
        let query = QueryParams {
            owner: Some(owner),
            order_uid: None,
            offset: Some(10),
            limit: Some(50),
        };
        let result = query.validate().unwrap();
        assert_eq!(result.owner, Some(owner));
        assert_eq!(result.offset, 10);
        assert_eq!(result.limit, 50);
    }

    #[test]
    fn query_validation_err() {
        let owner = Address::with_last_byte(1);
        let uid = OrderUid([1u8; 56]);
        let query = QueryParams {
            owner: Some(owner),
            order_uid: Some(uid),
            offset: None,
            limit: None,
        };
        assert!(query.validate().is_err());

        let query = QueryParams {
            owner: None,
            order_uid: None,
            offset: None,
            limit: None,
        };
        assert!(query.validate().is_err());

        // Test limit validation
        let query = QueryParams {
            owner: Some(owner),
            order_uid: None,
            offset: None,
            limit: Some(0),
        };
        assert!(matches!(
            query.validate(),
            Err(TradeFilterError::InvalidLimit(MIN_LIMIT, MAX_LIMIT))
        ));

        let query = QueryParams {
            owner: Some(owner),
            order_uid: None,
            offset: None,
            limit: Some(1001),
        };
        assert!(matches!(
            query.validate(),
            Err(TradeFilterError::InvalidLimit(MIN_LIMIT, MAX_LIMIT))
        ));
    }
}
