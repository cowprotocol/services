use crate::{
    order_validation::{PartialValidationError, ValidationError},
    orderbook::{AddOrderError, Orderbook},
};
use anyhow::Result;
use model::order::{OrderCreation, OrderUid};
use shared::api::{error, extract_payload, internal_error, ApiReply, IntoWarpReply};
use std::{convert::Infallible, sync::Arc};
use warp::reply::with_status;
use warp::{hyper::StatusCode, Filter, Rejection};

pub fn create_order_request() -> impl Filter<Extract = (OrderCreation,), Error = Rejection> + Clone
{
    warp::path!("orders")
        .and(warp::post())
        .and(extract_payload())
}

impl IntoWarpReply for PartialValidationError {
    fn into_warp_reply(self) -> ApiReply {
        match self {
            Self::UnsupportedBuyTokenDestination(dest) => with_status(
                error("UnsupportedBuyTokenDestination", format!("Type {dest:?}")),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedSellTokenSource(src) => with_status(
                error("UnsupportedSellTokenSource", format!("Type {src:?}")),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedOrderType => with_status(
                error(
                    "UnsupportedOrderType",
                    "This order type is currently not supported",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::Forbidden => with_status(
                error("Forbidden", "Forbidden, your account is deny-listed"),
                StatusCode::FORBIDDEN,
            ),
            Self::InsufficientValidTo => with_status(
                error(
                    "InsufficientValidTo",
                    "validTo is not far enough in the future",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::ExcessiveValidTo => with_status(
                error("ExcessiveValidTo", "validTo is too far into the future"),
                StatusCode::BAD_REQUEST,
            ),
            Self::TransferEthToContract => with_status(
                error(
                    "TransferEthToContract",
                    "Sending Ether to smart contract wallets is currently not supported",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::InvalidNativeSellToken => with_status(
                error(
                    "InvalidNativeSellToken",
                    "The chain's native token (Ether/xDai) cannot be used as the sell token",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::SameBuyAndSellToken => with_status(
                error(
                    "SameBuyAndSellToken",
                    "Buy token is the same as the sell token.",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedSignature => with_status(
                error("UnsupportedSignature", "signing scheme is not supported"),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedToken(token) => with_status(
                error("UnsupportedToken", format!("Token address {token:?}")),
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => with_status(
                internal_error(err.context("partial_validation")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    }
}

impl IntoWarpReply for ValidationError {
    fn into_warp_reply(self) -> ApiReply {
        match self {
            Self::Partial(pre) => pre.into_warp_reply(),
            Self::QuoteNotFound => with_status(
                error(
                    "QuoteNotFound",
                    "could not find quote with the specified ID",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::InvalidQuote => with_status(
                error(
                    "InvalidQuote",
                    "the quote with the specified ID does not match the order",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::PriceForQuote(err) => err.into_warp_reply(),
            Self::MissingFrom => with_status(
                error(
                    "MissingFrom",
                    "From address must be specified for on-chain signature",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::WrongOwner(owner) => with_status(
                error(
                    "WrongOwner",
                    format!("Address recovered from signature {owner} does not match from address"),
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::InsufficientBalance => with_status(
                error(
                    "InsufficientBalance",
                    "order owner must have funds worth at least x in his account",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::InsufficientAllowance => with_status(
                error(
                    "InsufficientAllowance",
                    "order owner must give allowance to VaultRelayer",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::InvalidSignature => with_status(
                error("InvalidSignature", "invalid signature"),
                StatusCode::BAD_REQUEST,
            ),
            Self::InsufficientFee => with_status(
                error("InsufficientFee", "Order does not include sufficient fee"),
                StatusCode::BAD_REQUEST,
            ),
            Self::SellAmountOverflow => with_status(
                error(
                    "SellAmountOverflow",
                    "Sell amount + fee amount must fit in U256",
                ),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            Self::TransferSimulationFailed => with_status(
                error(
                    "TransferSimulationFailed",
                    "sell token cannot be transferred",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::ZeroAmount => with_status(
                error("ZeroAmount", "Buy or sell amount is zero."),
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => with_status(
                internal_error(err.context("order_validation")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    }
}

impl IntoWarpReply for AddOrderError {
    fn into_warp_reply(self) -> ApiReply {
        match self {
            Self::OrderValidation(err) => err.into_warp_reply(),
            Self::DuplicatedOrder => with_status(
                error("DuplicatedOrder", "order already exists"),
                StatusCode::BAD_REQUEST,
            ),
            Self::Database(err) => with_status(
                internal_error(anyhow::Error::new(err).context("create_order")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    }
}

pub fn create_order_response(result: Result<OrderUid, AddOrderError>) -> ApiReply {
    match result {
        Ok(uid) => with_status(warp::reply::json(&uid), StatusCode::CREATED),
        Err(err) => err.into_warp_reply(),
    }
}

pub fn create_order(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    create_order_request().and_then(move |order_payload: OrderCreation| {
        let orderbook = orderbook.clone();
        async move {
            let quote_id = order_payload.quote_id;
            let result = orderbook.add_order(order_payload).await;
            if let Ok(order_uid) = result {
                tracing::debug!(%order_uid, ?quote_id, "order created");
            }
            Result::<_, Infallible>::Ok(create_order_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::order::{OrderCreation, OrderUid};
    use serde_json::json;
    use shared::api::response_body;
    use warp::{test::request, Reply};

    #[tokio::test]
    async fn create_order_request_ok() {
        let filter = create_order_request();
        let order_payload = OrderCreation::default();
        let request = request()
            .path("/orders")
            .method("POST")
            .header("content-type", "application/json")
            .json(&order_payload);
        let result = request.filter(&filter).await.unwrap();
        assert_eq!(result, order_payload);
    }

    #[tokio::test]
    async fn create_order_response_created() {
        let uid = OrderUid([1u8; 56]);
        let response = create_order_response(Ok(uid)).into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected= json!(
            "0x0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"
        );
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn create_order_response_duplicate() {
        let response = create_order_response(Err(AddOrderError::DuplicatedOrder)).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected_error =
            json!({"errorType": "DuplicatedOrder", "description": "order already exists"});
        assert_eq!(body, expected_error);
    }
}
