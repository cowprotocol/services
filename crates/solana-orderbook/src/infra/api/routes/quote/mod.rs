//! The quote endpoint: what one order would trade for right now.

pub mod dto;

use {
    super::mint::{ensure_settleable, token_mints},
    crate::infra::{
        api::{Sponsoring, State, ValidationParameters, error, extract},
        db,
        quoter,
    },
    axum::{Json, http::StatusCode},
    chrono::Utc,
    cow_settlement_interface::data::intent::ENCODED_NATIVE_SOL_TRANSFER,
    database::{byte_array::ByteArray, solana::OrderKind},
    solana_sdk::pubkey::Pubkey,
    spl_token_interface::native_mint,
    std::time::Duration,
};

/// How long a quoted order stays valid when the request names no validity.
const DEFAULT_VALIDITY: Duration = Duration::from_secs(30 * 60);

/// TODO: fail the quote on a store error instead, like the EVM orderbook,
/// once fee policies consume the link and the id becomes load-bearing.
const SAVE_TIMEOUT: Duration = Duration::from_secs(3);

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
    validate(&request, valid_to, now_secs, &state.validation())?;
    if let Some(sponsoring) = state.sponsoring() {
        check_mints(sponsoring, &request).await?;
    }

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
        // Every driver failure answers as no liquidity, the EVM mapping for
        // estimator errors.
        .map_err(|quoter::Error::NoQuotes| {
            error::reply(StatusCode::NOT_FOUND, "NoLiquidity", "no route found")
        })?;
    check_native_payout(state.sponsoring(), &request, quoted.buy_amount).await?;

    let expiration = now + state.quote_expiry();
    // A failed insert answers without an id instead of failing the quote,
    // since the fees are not yet implemented and stored quote are not mandatory
    // at the moment.
    let quote = db::Quote {
        sell_token: ByteArray(request.sell_token.to_bytes()),
        buy_token: ByteArray(request.buy_token.to_bytes()),
        sell_amount: quoted.sell_amount,
        buy_amount: quoted.buy_amount,
        kind: match kind {
            dto::Kind::Sell => OrderKind::Sell,
            dto::Kind::Buy => OrderKind::Buy,
        },
        solver: ByteArray(quoted.solver.to_bytes()),
        expiration,
    };
    let save = db::save_quote(state.pool(), &quote);
    let id = match tokio::time::timeout(SAVE_TIMEOUT, save).await {
        Ok(Ok(id)) => Some(id),
        Ok(Err(err)) => {
            tracing::error!(?err, "quote insert failed");
            None
        }
        Err(_) => {
            tracing::error!("quote insert timed out");
            None
        }
    };

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
        expiration,
        id,
        verified: false,
        funder: state.sponsoring().map(|sponsoring| sponsoring.funder),
    }))
}

/// Reject a mint the settlement program cannot move. The chain read goes
/// through the sponsoring RPC client, so the check is skipped without
/// sponsoring and when the read fails: placement and the autopilot check the
/// mints again.
async fn check_mints(sponsoring: &Sponsoring, request: &dto::Request) -> Result<(), error::Reply> {
    let mints: Vec<Pubkey> = token_mints(request.sell_token, request.buy_token).collect();
    let lookup = sponsoring.mints.lookup(mints.iter().copied());
    match sponsoring.rpc.multiple_accounts(lookup.unread()).await {
        Ok(accounts) => ensure_settleable(&lookup.resolve(&accounts), mints),
        Err(err) => {
            tracing::warn!(?err, "mint lookup failed, quoting unchecked");
            Ok(())
        }
    }
}

/// Reject a native SOL buy whose payout would leave its wallet under the
/// rent-exempt minimum, checked as fill-or-kill since the quote names no fill
/// policy. Only a payout an empty wallet cannot take reads the wallet, through
/// the sponsoring RPC client. Without sponsoring, or when the read fails, the
/// wallet counts as empty.
async fn check_native_payout(
    sponsoring: Option<&Sponsoring>,
    request: &dto::Request,
    buy_amount: u64,
) -> Result<(), error::Reply> {
    if request.buy_token != ENCODED_NATIVE_SOL_TRANSFER
        || super::receivable_native_payout(None, buy_amount, false)
    {
        return Ok(());
    }
    let wallet = request.receiver.unwrap_or(request.from);
    let account = match sponsoring {
        Some(sponsoring) => match sponsoring.rpc.multiple_accounts([wallet]).await {
            Ok(mut accounts) => accounts.remove(&wallet),
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "native buy wallet lookup failed, assuming it is empty"
                );
                None
            }
        },
        None => None,
    };
    if super::receivable_native_payout(account.as_ref(), buy_amount, false) {
        return Ok(());
    }
    Err(error::reply(
        StatusCode::BAD_REQUEST,
        "InvalidNativeBuy",
        "a native SOL buy must leave its wallet rent-exempt",
    ))
}

/// The checks an order must pass before it is worth quoting.
fn validate(
    request: &dto::Request,
    valid_to: u32,
    now_secs: u32,
    validation: &ValidationParameters,
) -> Result<(), error::Reply> {
    if same_token(&request.sell_token, &request.buy_token) {
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
    if valid_to < now_secs.saturating_add(validation.min_validity.as_secs() as u32) {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "InsufficientValidTo",
            "validTo is not far enough in the future",
        ));
    }
    if valid_to > now_secs.saturating_add(validation.max_validity.as_secs() as u32) {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "ExcessiveValidTo",
            "validTo is too far into the future",
        ));
    }
    Ok(())
}

/// Whether a quote trades a token for itself. The System Program ID stands
/// for native SOL only on the buy side, where it counts as wSOL. A SOL sell
/// names the wSOL mint.
fn same_token(sell: &Pubkey, buy: &Pubkey) -> bool {
    sell == buy || (*sell == native_mint::ID && *buy == ENCODED_NATIVE_SOL_TRANSFER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_sol_buys_count_as_wsol() {
        let mint = Pubkey::new_unique();
        assert!(same_token(&mint, &mint));
        assert!(!same_token(&mint, &Pubkey::new_unique()));
        assert!(same_token(&native_mint::ID, &ENCODED_NATIVE_SOL_TRANSFER));
        assert!(!same_token(&mint, &ENCODED_NATIVE_SOL_TRANSFER));
        // As a sell token the System Program ID is not SOL. A SOL sell names
        // the wSOL mint.
        assert!(!same_token(&ENCODED_NATIVE_SOL_TRANSFER, &native_mint::ID));
    }
}
