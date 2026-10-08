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
    cow_settlement_interface::{
        data::intent::ENCODED_NATIVE_SOL_TRANSFER,
        token_program::TokenProgram,
    },
    database::{byte_array::ByteArray, solana::OrderKind},
    solana_sdk::pubkey::Pubkey,
    solana_token::{ata_rent, max_ata_rent, receivable_token_account},
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id,
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
    let buy_program = match state.sponsoring() {
        Some(sponsoring) => check_mints(sponsoring, &request).await?,
        None => None,
    };

    let (kind, amount) = request.side.kind_and_amount();
    let order = quoter::Order {
        sell_token: request.sell_token,
        buy_token: request.buy_token,
        amount,
        kind: match kind {
            dto::Kind::Sell => quoter::Kind::Sell,
            dto::Kind::Buy => quoter::Kind::Buy,
        },
    };
    let (quoted, execution_cost_lamports) = tokio::join!(
        state.quoter().quote(&order),
        buy_account_rent(state.sponsoring(), &request, buy_program),
    );
    // Every driver failure answers as no liquidity, the EVM mapping for
    // estimator errors.
    let quoted = quoted.map_err(|quoter::Error::NoQuotes| {
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
            execution_cost_lamports,
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

/// Reject a mint the settlement program cannot move, and return the buy
/// mint's token program: `None` for a native SOL buy or when the read fails.
/// The chain read goes through the sponsoring RPC client, so the check is
/// skipped without sponsoring and when the read fails: placement and the
/// autopilot check the mints again.
async fn check_mints(
    sponsoring: &Sponsoring,
    request: &dto::Request,
) -> Result<Option<TokenProgram>, error::Reply> {
    let mints: Vec<Pubkey> = token_mints(request.sell_token, request.buy_token).collect();
    let lookup = sponsoring.mints.lookup(mints.iter().copied());
    match sponsoring.rpc.multiple_accounts(lookup.unread()).await {
        Ok(accounts) => {
            let verdicts = lookup.resolve(&accounts);
            ensure_settleable(&verdicts, mints)?;
            Ok(verdicts
                .get(&request.buy_token)
                .copied()
                .and_then(Result::ok))
        }
        Err(err) => {
            tracing::warn!(?err, "mint lookup failed, quoting unchecked");
            Ok(None)
        }
    }
}

/// The rent of the quoted order's buy token account, zero when the payout
/// lands in an existing one: the `receiver` itself, or the associated token
/// account of the `receiver`, or of `from` without one. A native SOL buy pays
/// out to a wallet, and an anonymous quote without a `receiver` names no
/// account to read: neither owes rent. The read goes through the sponsoring
/// RPC client. Without it, without the buy mint's token program, or when the
/// read fails, the account costs the most a settleable mint can need.
async fn buy_account_rent(
    sponsoring: Option<&Sponsoring>,
    request: &dto::Request,
    buy_program: Option<TokenProgram>,
) -> u64 {
    if request.buy_token == ENCODED_NATIVE_SOL_TRANSFER
        || request.receiver.unwrap_or(request.from) == Pubkey::default()
    {
        return 0;
    }
    let (Some(sponsoring), Some(program)) = (sponsoring, buy_program) else {
        return max_ata_rent();
    };
    let [recipient, ata] = buy_token_account_candidates(request, program);
    let accounts = match sponsoring
        .rpc
        .multiple_accounts([recipient, ata, request.buy_token])
        .await
    {
        Ok(accounts) => accounts,
        Err(err) => {
            tracing::warn!(?err, "buy token account lookup failed, charging its rent");
            return max_ata_rent();
        }
    };
    let receives = |key| {
        accounts
            .get(key)
            .is_some_and(|account| receivable_token_account(account, &request.buy_token))
    };
    if receives(&recipient) || receives(&ata) {
        0
    } else {
        accounts
            .get(&request.buy_token)
            .and_then(ata_rent)
            .unwrap_or_else(max_ata_rent)
    }
}

/// The accounts that may be the quoted order's buy token account: the
/// `receiver`, or `from` without one, then its associated token account of
/// the buy mint under `program`.
fn buy_token_account_candidates(request: &dto::Request, program: TokenProgram) -> [Pubkey; 2] {
    let recipient = request.receiver.unwrap_or(request.from);
    let ata = get_associated_token_address_with_program_id(
        &recipient,
        &request.buy_token,
        &program.address(),
    );
    [recipient, ata]
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
    use {
        super::*,
        cow_solana_rpc::{Mocks, RpcRequest, SolanaRPC},
        solana_sdk::account::Account,
        solana_testlib::{
            account_json,
            classic_mint,
            multiple_accounts_json,
            token_2022_mint,
            token_account_json,
        },
    };

    fn request(from: Pubkey, buy_token: Pubkey, receiver: Option<Pubkey>) -> dto::Request {
        dto::Request {
            from,
            sell_token: native_mint::ID,
            buy_token,
            receiver,
            side: dto::Side::Sell {
                sell_amount: dto::SellAmount::BeforeFee { value: 1 },
            },
            validity: None,
            app_data: None,
        }
    }

    /// A sponsoring deployment whose RPC answers the account read with
    /// `accounts`.
    fn sponsoring(accounts: serde_json::Value) -> Sponsoring {
        Sponsoring {
            funder: Pubkey::new_unique(),
            settlement_program: cow_settlement_interface::id(),
            rpc: SolanaRPC::new_mock_with_mocks(Mocks::from([(
                RpcRequest::GetMultipleAccounts,
                accounts,
            )])),
            max_priority_fee_lamports: 100_000,
            mints: Default::default(),
        }
    }

    /// The account read answers the recipient, its associated token account,
    /// then the buy mint. The payout lands in a recipient that is a token
    /// account of the mint, or else in the associated token account. Owing
    /// the mint's rent: a missing associated token account, a wallet holding
    /// only lamports at its address, or another mint's account there.
    #[tokio::test]
    async fn a_missing_buy_token_account_owes_its_rent() {
        let (owner, mint, receiver) = (
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        let wallet = account_json(&Account {
            lamports: 1_000_000_000,
            owner: solana_system_interface::program::ID,
            ..Default::default()
        });
        let token_account = token_account_json(&mint, &owner);
        let other_mint_account = token_account_json(&Pubkey::new_unique(), &owner);
        let classic = account_json(&classic_mint(6));
        let token_2022 = account_json(&token_2022_mint(&[], |_| {}));
        let null = serde_json::Value::Null;
        for (receiver, accounts, program, cost) in [
            (
                None,
                [wallet.clone(), token_account.clone(), classic.clone()],
                TokenProgram::SplToken,
                0,
            ),
            (
                None,
                [wallet.clone(), null.clone(), classic.clone()],
                TokenProgram::SplToken,
                2_039_280,
            ),
            (
                None,
                [wallet.clone(), null.clone(), token_2022.clone()],
                TokenProgram::Token2022,
                2_074_080,
            ),
            (
                None,
                [wallet.clone(), wallet.clone(), classic.clone()],
                TokenProgram::SplToken,
                2_039_280,
            ),
            (
                None,
                [wallet.clone(), other_mint_account.clone(), classic.clone()],
                TokenProgram::SplToken,
                2_039_280,
            ),
            (
                None,
                [wallet.clone(), null.clone(), null.clone()],
                TokenProgram::SplToken,
                max_ata_rent(),
            ),
            (
                Some(receiver),
                [token_account.clone(), null.clone(), classic.clone()],
                TokenProgram::SplToken,
                0,
            ),
            (
                Some(receiver),
                [wallet.clone(), token_account.clone(), classic.clone()],
                TokenProgram::SplToken,
                0,
            ),
            (
                Some(receiver),
                [wallet.clone(), null.clone(), classic.clone()],
                TokenProgram::SplToken,
                2_039_280,
            ),
            (
                Some(receiver),
                [null.clone(), null.clone(), classic.clone()],
                TokenProgram::SplToken,
                2_039_280,
            ),
        ] {
            let sponsoring = sponsoring(multiple_accounts_json(accounts.clone()));
            assert_eq!(
                buy_account_rent(
                    Some(&sponsoring),
                    &request(owner, mint, receiver),
                    Some(program)
                )
                .await,
                cost,
                "{receiver:?} {accounts:?} {program:?}"
            );
        }
    }

    #[test]
    fn the_buy_token_account_is_the_recipient_or_its_associated_token_account() {
        let (from, mint, receiver) = (
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        assert_eq!(
            buy_token_account_candidates(
                &request(from, mint, Some(receiver)),
                TokenProgram::Token2022
            ),
            [
                receiver,
                get_associated_token_address_with_program_id(
                    &receiver,
                    &mint,
                    &spl_token_2022_interface::ID
                ),
            ]
        );
        assert_eq!(
            buy_token_account_candidates(&request(from, mint, None), TokenProgram::SplToken),
            [
                from,
                get_associated_token_address_with_program_id(
                    &from,
                    &mint,
                    &spl_token_interface::ID
                ),
            ]
        );
    }

    #[tokio::test]
    async fn anonymous_quotes_and_native_buys_owe_no_rent() {
        let owner = Pubkey::new_unique();
        for request in [
            request(Pubkey::default(), Pubkey::new_unique(), None),
            request(owner, ENCODED_NATIVE_SOL_TRANSFER, None),
            request(
                owner,
                ENCODED_NATIVE_SOL_TRANSFER,
                Some(Pubkey::new_unique()),
            ),
        ] {
            assert_eq!(
                buy_account_rent(None, &request, None).await,
                0,
                "{request:?}"
            );
        }
    }

    /// Without the sponsoring RPC, without the buy mint's token program, or
    /// when the read fails, the buy token account costs the most a settleable
    /// mint can need, even for an SPL Token mint. An anonymous quote naming a
    /// `receiver` has an account to price.
    #[tokio::test]
    async fn an_unread_buy_token_account_owes_the_largest_rent() {
        let anonymous = request(
            Pubkey::default(),
            Pubkey::new_unique(),
            Some(Pubkey::new_unique()),
        );
        assert_eq!(
            buy_account_rent(None, &anonymous, None).await,
            max_ata_rent()
        );
        let request = request(Pubkey::new_unique(), Pubkey::new_unique(), None);
        let exists = sponsoring(multiple_accounts_json([
            serde_json::Value::Null,
            token_account_json(&request.buy_token, &request.from),
        ]));
        let failing = sponsoring(serde_json::json!("not an account list"));
        for (sponsoring, program) in [
            (None, Some(TokenProgram::SplToken)),
            (Some(&exists), None),
            (Some(&failing), Some(TokenProgram::SplToken)),
        ] {
            assert_eq!(
                buy_account_rent(sponsoring, &request, program).await,
                max_ata_rent()
            );
        }
    }

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
