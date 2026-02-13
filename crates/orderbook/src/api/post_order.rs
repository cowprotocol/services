use {
    crate::{
        api::{AppState, error},
        orderbook::{AddOrderError, OrderReplacementError},
    },
    axum::{
        Json,
        body,
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    model::{
        order::{AppdataFromMismatch, OrderCreation},
        signature,
    },
    shared::order_validation::{
        AppDataValidationError,
        OrderValidToError,
        PartialValidationError,
        ValidationError,
    },
    std::sync::Arc,
};

pub async fn post_order_handler(State(state): State<Arc<AppState>>, body: body::Bytes) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let order = match serde_json::from_slice::<OrderCreation>(&body) {
        Ok(order) => order,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };

    state
        .orderbook
        .add_order(order.clone())
        .await
        .map(|(order_uid, quote_metadata)| {
            let quote_id = quote_metadata.as_ref().and_then(|q| q.id);
            let quote_solver = quote_metadata.as_ref().map(|q| q.solver);
            tracing::debug!(%order_uid, ?quote_id, ?quote_solver, "order created");
            (StatusCode::CREATED, Json(order_uid))
        })
        .inspect_err(|err| {
            tracing::debug!(?order, ?err, "error creating order");
        })
        .into_response()
}

pub struct PartialValidationErrorWrapper(pub PartialValidationError);
impl IntoResponse for PartialValidationErrorWrapper {
    fn into_response(self) -> Response {
        match self.0 {
            PartialValidationError::UnsupportedBuyTokenDestination(dest) => (
                StatusCode::BAD_REQUEST,
                error("UnsupportedBuyTokenDestination", format!("Type {dest:?}")),
            )
                .into_response(),
            PartialValidationError::UnsupportedSellTokenSource(src) => (
                StatusCode::BAD_REQUEST,
                error("UnsupportedSellTokenSource", format!("Type {src:?}")),
            )
                .into_response(),
            PartialValidationError::UnsupportedOrderType => (
                StatusCode::BAD_REQUEST,
                error(
                    "UnsupportedOrderType",
                    "This order type is currently not supported",
                ),
            )
                .into_response(),
            PartialValidationError::Forbidden => (
                StatusCode::FORBIDDEN,
                error("Forbidden", "Forbidden, your account is deny-listed"),
            )
                .into_response(),
            PartialValidationError::ValidTo(OrderValidToError::Insufficient) => (
                StatusCode::BAD_REQUEST,
                error(
                    "InsufficientValidTo",
                    "validTo is not far enough in the future",
                ),
            )
                .into_response(),
            PartialValidationError::ValidTo(OrderValidToError::Excessive) => (
                StatusCode::BAD_REQUEST,
                error("ExcessiveValidTo", "validTo is too far into the future"),
            )
                .into_response(),
            PartialValidationError::InvalidNativeSellToken => (
                StatusCode::BAD_REQUEST,
                error(
                    "InvalidNativeSellToken",
                    "The chain's native token (Ether/xDai) cannot be used as the sell token",
                ),
            )
                .into_response(),
            PartialValidationError::SameBuyAndSellToken => (
                StatusCode::BAD_REQUEST,
                error(
                    "SameBuyAndSellToken",
                    "Buy token is the same as the sell token.",
                ),
            )
                .into_response(),
            PartialValidationError::UnsupportedToken { token, reason } => (
                StatusCode::BAD_REQUEST,
                error(
                    "UnsupportedToken",
                    format!("Token {token:?} is unsupported: {reason}"),
                ),
            )
                .into_response(),
            PartialValidationError::Other(err) => {
                tracing::error!(?err, "PartialValidatonError");
                crate::api::internal_error_reply()
            }
        }
    }
}

pub struct AppDataValidationErrorWrapper(pub AppDataValidationError);
impl IntoResponse for AppDataValidationErrorWrapper {
    fn into_response(self) -> Response {
        match self.0 {
            AppDataValidationError::Invalid(err) => (
                StatusCode::BAD_REQUEST,
                error("InvalidAppData", format!("{err:?}")),
            )
                .into_response(),
            AppDataValidationError::Mismatch { provided, actual } => (
                StatusCode::BAD_REQUEST,
                error(
                    "AppDataHashMismatch",
                    format!(
                        "calculated app data hash {actual:?} doesn't match order app data field \
                         {provided:?}",
                    ),
                ),
            )
                .into_response(),
        }
    }
}

pub struct ValidationErrorWrapper(ValidationError);
impl IntoResponse for ValidationErrorWrapper {
    fn into_response(self) -> Response {
        match self.0 {
            ValidationError::Partial(pre) => PartialValidationErrorWrapper(pre).into_response(),
            ValidationError::AppData(err) => AppDataValidationErrorWrapper(err).into_response(),
            ValidationError::PriceForQuote(err) => {
                super::PriceEstimationErrorWrapper(err).into_response()
            }
            ValidationError::MissingFrom => (
                StatusCode::BAD_REQUEST,
                error(
                    "MissingFrom",
                    "From address must be specified for on-chain signature",
                ),
            )
                .into_response(),
            ValidationError::AppdataFromMismatch(AppdataFromMismatch {
                from,
                app_data_signer,
            }) => (
                StatusCode::BAD_REQUEST,
                error(
                    "AppdataFromMismatch",
                    format!(
                        "from address {from:?} cannot be different from metadata.signer \
                         {app_data_signer:?} specified in the app data"
                    ),
                ),
            )
                .into_response(),
            ValidationError::WrongOwner(signature::Recovered { message, signer }) => (
                StatusCode::BAD_REQUEST,
                error(
                    "WrongOwner",
                    format!(
                        "recovered signer {signer:?} from signing hash {message:?} does not match \
                         from address"
                    ),
                ),
            )
                .into_response(),
            ValidationError::InvalidEip1271Signature(hash) => (
                StatusCode::BAD_REQUEST,
                error(
                    "InvalidEip1271Signature",
                    format!("signature for computed order hash {hash:?} is not valid"),
                ),
            )
                .into_response(),
            ValidationError::InsufficientBalance => (
                StatusCode::BAD_REQUEST,
                error(
                    "InsufficientBalance",
                    "order owner must have funds worth at least x in his account",
                ),
            )
                .into_response(),
            ValidationError::InsufficientAllowance => (
                StatusCode::BAD_REQUEST,
                error(
                    "InsufficientAllowance",
                    "order owner must give allowance to VaultRelayer",
                ),
            )
                .into_response(),
            ValidationError::InvalidSignature => (
                StatusCode::BAD_REQUEST,
                error("InvalidSignature", "invalid signature"),
            )
                .into_response(),
            ValidationError::NonZeroFee => (
                StatusCode::BAD_REQUEST,
                error("NonZeroFee", "Fee must be zero"),
            )
                .into_response(),
            ValidationError::SellAmountOverflow => (
                StatusCode::INTERNAL_SERVER_ERROR,
                error(
                    "SellAmountOverflow",
                    "Sell amount + fee amount must fit in U256",
                ),
            )
                .into_response(),
            ValidationError::TransferSimulationFailed => (
                StatusCode::BAD_REQUEST,
                error(
                    "TransferSimulationFailed",
                    "sell token cannot be transferred",
                ),
            )
                .into_response(),
            ValidationError::QuoteNotVerified => (
                StatusCode::BAD_REQUEST,
                error(
                    "QuoteNotVerified",
                    "No quote for this trade could be verified to be accurate. Aborting the order \
                     creation since it will likely not be executed.",
                ),
            )
                .into_response(),
            ValidationError::ZeroAmount => (
                StatusCode::BAD_REQUEST,
                error("ZeroAmount", "Buy or sell amount is zero."),
            )
                .into_response(),
            ValidationError::IncompatibleSigningScheme => (
                StatusCode::BAD_REQUEST,
                error(
                    "IncompatibleSigningScheme",
                    "Signing scheme is not compatible with order placement method.",
                ),
            )
                .into_response(),
            ValidationError::TooManyLimitOrders => (
                StatusCode::BAD_REQUEST,
                error("TooManyLimitOrders", "Too many limit orders"),
            )
                .into_response(),
            ValidationError::TooMuchGas => (
                StatusCode::BAD_REQUEST,
                error("TooMuchGas", "Executing order requires too many gas units"),
            )
                .into_response(),

            ValidationError::Other(err) => {
                tracing::error!(?err, "ValidationErrorWrapper");
                crate::api::internal_error_reply()
            }
        }
    }
}

impl IntoResponse for AddOrderError {
    fn into_response(self) -> Response {
        match self {
            Self::OrderValidation(err) => ValidationErrorWrapper(err).into_response(),
            Self::DuplicatedOrder => (
                StatusCode::BAD_REQUEST,
                error("DuplicatedOrder", "order already exists"),
            )
                .into_response(),
            Self::Database(err) => {
                tracing::error!(?err, "AddOrderError");
                crate::api::internal_error_reply()
            }
            err @ AddOrderError::AppDataMismatch { .. } => {
                tracing::error!(
                    ?err,
                    "An order with full app data passed validation but then failed to be inserted \
                     because we already stored different full app data for the same contract app \
                     data. This should be impossible."
                );
                crate::api::internal_error_reply()
            }
            AddOrderError::OrderNotFound(err) => err.into_response(),
            AddOrderError::InvalidAppData(err) => (
                StatusCode::BAD_REQUEST,
                super::error("InvalidAppData", err.to_string()),
            )
                .into_response(),
            AddOrderError::InvalidReplacement(err) => err.into_response(),
            AddOrderError::MetadataSerializationFailed(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                super::error("MetadataSerializationFailed", err.to_string()),
            )
                .into_response(),
        }
    }
}

impl IntoResponse for OrderReplacementError {
    fn into_response(self) -> Response {
        match self {
            OrderReplacementError::InvalidSignature => (
                StatusCode::BAD_REQUEST,
                super::error("InvalidSignature", "Malformed signature"),
            )
                .into_response(),
            OrderReplacementError::WrongOwner => (
                StatusCode::UNAUTHORIZED,
                super::error("WrongOwner", "Old and new orders have different signers"),
            )
                .into_response(),
            OrderReplacementError::OldOrderActivelyBidOn => (
                StatusCode::BAD_REQUEST,
                super::error(
                    "OldOrderActivelyBidOn",
                    "The old order is actively beign bid on in recent auctions",
                ),
            )
                .into_response(),
            OrderReplacementError::Other(err) => {
                tracing::error!(?err, "replace_order");
                crate::api::internal_error_reply()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::api::response_body, model::order::OrderUid, serde_json::json};

    type Result = std::result::Result<(StatusCode, Json<OrderUid>), AddOrderError>;

    #[tokio::test]
    async fn create_order_response_created() {
        let uid = OrderUid([1u8; 56]);
        let response = Result::Ok((StatusCode::CREATED, Json(uid))).into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected = json!(
            "0x0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"
        );
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn create_order_response_duplicate() {
        let response = Result::Err(AddOrderError::DuplicatedOrder).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected_error =
            json!({"errorType": "DuplicatedOrder", "description": "order already exists"});
        assert_eq!(body, expected_error);
    }
}
