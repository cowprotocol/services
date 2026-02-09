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
    fn trade_filter(&self) -> TradeFilter {
        TradeFilter {
            order_uid: self.order_uid,
            owner: self.owner,
        }
    }

    fn validate(&self) -> Result<TradeFilter, TradeFilterError> {
        match (self.order_uid.as_ref(), self.owner.as_ref()) {
            (Some(_), None) | (None, Some(_)) => Ok(self.trade_filter()),
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
        let result = query.validate().unwrap();
        assert_eq!(result.owner, Some(owner));
        assert_eq!(result.order_uid, None);

        let uid = OrderUid([1u8; 56]);
        let query = QueryParams {
            owner: None,
            order_uid: Some(uid),
        };
        let result = query.validate().unwrap();
        assert_eq!(result.owner, None);
        assert_eq!(result.order_uid, Some(uid));
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
