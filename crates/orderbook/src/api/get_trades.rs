use {
    crate::{
        api::{AppState, error},
        database::trades::{TradeFilter, TradeRetrieving},
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
}

#[derive(Debug, Eq, PartialEq)]
enum TradeFilterError {
    InvalidFilter(String),
}

impl QueryParams {
    fn validate(&self) -> Result<TradeFilter, TradeFilterError> {
        match (self.owner, self.order_uid) {
            (Some(owner), None) => Ok(TradeFilter::Owner(owner)),
            (None, Some(uid)) => Ok(TradeFilter::OrderUid(uid)),
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
    };

    let result = state
        .database_read
        .trades(&trade_filter)
        .await
        .context("get_trades");
    match result {
        Ok(reply) => Json(reply).into_response(),
        Err(err) => {
            tracing::error!(?err, "get_trades");
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
        };
        assert_eq!(query.validate().unwrap(), TradeFilter::Owner(owner));

        let uid = OrderUid([1u8; 56]);
        let query = QueryParams {
            owner: None,
            order_uid: Some(uid),
        };
        assert_eq!(query.validate().unwrap(), TradeFilter::OrderUid(uid));
    }

    #[test]
    fn query_validation_err() {
        let owner = Address::with_last_byte(1);
        let uid = OrderUid([1u8; 56]);
        let query = QueryParams {
            owner: Some(owner),
            order_uid: Some(uid),
        };
        assert!(query.validate().is_err());

        let query = QueryParams {
            owner: None,
            order_uid: None,
        };
        assert!(query.validate().is_err());
    }
}
