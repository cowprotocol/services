//! The quote endpoint: what one order would trade for right now.

pub mod dto;

use {
    crate::infra::{
        api::{State, error},
        quoter,
    },
    axum::{Json, http::StatusCode},
    bigdecimal::BigDecimal,
    chrono::Utc,
    std::time::Duration,
};

/// How long a quoted order stays valid when the request names no validity.
const DEFAULT_VALIDITY: Duration = Duration::from_secs(30 * 60);

/// How long the quoted amounts are honored for.
const QUOTE_EXPIRY: Duration = Duration::from_secs(60);

/// Handle `POST /api/v1/quote`.
pub async fn quote(
    state: axum::extract::State<State>,
    Json(request): Json<dto::Request>,
) -> Result<Json<dto::Response>, error::Reply> {
    if request.sell_token == request.buy_token {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "SameBuyAndSellToken",
            "Buy and sell token must differ.",
        ));
    }
    let (kind, amount) = request.side.kind_and_amount();
    if amount == 0 {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "ZeroAmount",
            "The quoted amount must be positive.",
        ));
    }

    let quoted = state
        .quoter()
        .quote(&quoter::Order {
            sell_token: request.sell_token,
            buy_token: request.buy_token,
            amount,
            kind: match kind {
                dto::Kind::Sell => quoter::Kind::Sell,
                dto::Kind::Buy => quoter::Kind::Buy,
            },
        })
        .await
        .map_err(|err| match err {
            quoter::Error::NoRoute => error::reply(
                StatusCode::NOT_FOUND,
                "NoLiquidity",
                "No route was found for the requested pair.",
            ),
            err => {
                tracing::warn!(?err, "quote lookup failed");
                error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
            }
        })?;

    let now = Utc::now();
    let valid_to = match request.validity {
        Some(dto::Validity::ValidTo(valid_to)) => valid_to,
        Some(dto::Validity::ValidFor(seconds)) => u32::try_from(now.timestamp())
            .unwrap_or(u32::MAX)
            .saturating_add(seconds),
        None => u32::try_from(now.timestamp())
            .unwrap_or(u32::MAX)
            .saturating_add(DEFAULT_VALIDITY.as_secs() as u32),
    };

    Ok(Json(dto::Response {
        quote: dto::Quote {
            sell_token: request.sell_token,
            buy_token: request.buy_token,
            receiver: request.receiver,
            sell_amount: BigDecimal::from(quoted.sell_amount),
            buy_amount: BigDecimal::from(quoted.buy_amount),
            valid_to,
            app_data: request.app_data,
            fee_amount: BigDecimal::from(0),
            kind,
            partially_fillable: false,
        },
        from: request.from,
        expiration: now + QUOTE_EXPIRY,
        id: None,
        verified: false,
    }))
}
