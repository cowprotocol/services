//! The quote endpoint: what one order would trade for right now.

pub mod dto;

use {
    crate::infra::{
        api::{QuoteLimits, State, error, extract},
        quoter,
    },
    axum::{Json, http::StatusCode},
    chrono::Utc,
    std::time::Duration,
};

/// How long a quoted order stays valid when the request names no validity.
const DEFAULT_VALIDITY: Duration = Duration::from_secs(30 * 60);

/// Handle `POST /api/v1/quote`.
pub async fn quote(
    state: axum::extract::State<State>,
    extract::Json(request): extract::Json<dto::Request>,
) -> Result<Json<dto::Response>, error::Reply> {
    let now = Utc::now();
    let now_secs = u32::try_from(now.timestamp()).unwrap_or(u32::MAX);
    // Fixed before validation so the value checked is the value returned.
    let valid_to = match request.validity {
        Some(dto::Validity::ValidTo(valid_to)) => valid_to,
        Some(dto::Validity::ValidFor(seconds)) => now_secs.saturating_add(seconds),
        None => now_secs.saturating_add(DEFAULT_VALIDITY.as_secs() as u32),
    };
    let limits = state.quote_limits();
    validate(&request, valid_to, now_secs, &limits)?;

    let (kind, amount) = request.side.kind_and_amount();
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
        .map_err(|err| {
            // A driver that fails to answer is a driver with no route, not a
            // fault of this service.
            tracing::warn!(?err, "quote lookup failed");
            error::reply(StatusCode::NOT_FOUND, "NoLiquidity", "no route found")
        })?;

    Ok(Json(dto::Response {
        quote: dto::Quote {
            sell_token: request.sell_token,
            buy_token: request.buy_token,
            receiver: request.receiver,
            sell_amount: quoted.sell_amount,
            buy_amount: quoted.buy_amount,
            valid_to,
            app_data: request.app_data,
            fee_amount: 0,
            kind,
            partially_fillable: false,
        },
        from: request.from,
        expiration: now + limits.quote_expiry,
        id: None,
        verified: false,
    }))
}

/// The checks an order must pass before it is worth quoting.
fn validate(
    request: &dto::Request,
    valid_to: u32,
    now_secs: u32,
    limits: &QuoteLimits,
) -> Result<(), error::Reply> {
    if request.sell_token == request.buy_token {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "SameBuyAndSellToken",
            "Buy token is the same as the sell token.",
        ));
    }
    if request.side.kind_and_amount().1 == 0 {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "ZeroAmount",
            "Buy or sell amount is zero.",
        ));
    }
    if valid_to < now_secs.saturating_add(limits.min_validity.as_secs() as u32) {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "InsufficientValidTo",
            "validTo is not far enough in the future",
        ));
    }
    if valid_to > now_secs.saturating_add(limits.max_validity.as_secs() as u32) {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "ExcessiveValidTo",
            "validTo is too far into the future",
        ));
    }
    Ok(())
}
