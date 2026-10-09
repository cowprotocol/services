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
    solana_sdk::{
        account::{Account, from_account},
        pubkey::Pubkey,
        rent::Rent,
        sysvar,
    },
    solana_token::{
        ata_rent,
        max_ata_rent,
        receivable_token_account,
        receivable_token_account_owner,
    },
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id,
    spl_token_interface::native_mint,
    std::{collections::HashMap, time::Duration},
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
    let buy_program = check_mints(state.sponsoring(), &request).await?;

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
    let execution_cost_lamports = execution_cost_lamports?;
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

/// Reject a mint the settlement program cannot move, and tell the buy mint's
/// token program. The chain read goes through the sponsoring RPC client, so
/// the check is skipped without sponsoring and when the read fails: placement
/// and the autopilot check the mints again.
async fn check_mints(
    sponsoring: Option<&Sponsoring>,
    request: &dto::Request,
) -> Result<BuyProgram, error::Reply> {
    let Some(sponsoring) = sponsoring else {
        return Ok(BuyProgram::new(request, None));
    };
    let mints: Vec<Pubkey> = token_mints(request.sell_token, request.buy_token).collect();
    let lookup = sponsoring.mints.lookup(mints.iter().copied());
    match sponsoring.rpc.multiple_accounts(lookup.unread()).await {
        Ok(accounts) => {
            let verdicts = lookup.resolve(&accounts);
            ensure_settleable(&verdicts, mints)?;
            let program = verdicts
                .get(&request.buy_token)
                .copied()
                .and_then(Result::ok);
            Ok(BuyProgram::new(request, program))
        }
        Err(err) => {
            tracing::warn!(?err, "mint lookup failed, quoting unchecked");
            Ok(BuyProgram::new(request, None))
        }
    }
}

/// The buy mint's token program, as far as the quote's mint check knows it.
#[derive(Clone, Copy, Debug)]
enum BuyProgram {
    /// A native SOL buy pays out to a wallet, not a token account.
    Native,
    Known(TokenProgram),
    /// No sponsoring RPC to read the mints, or the read failed.
    Unread,
}

impl BuyProgram {
    fn new(request: &dto::Request, program: Option<TokenProgram>) -> Self {
        if request.buy_token == ENCODED_NATIVE_SOL_TRANSFER {
            Self::Native
        } else {
            program.map_or(Self::Unread, Self::Known)
        }
    }
}

/// The rent of the quoted order's buy token account, zero when the payout
/// lands in an existing one. An existing account that cannot take the payout
/// is refused: the placement's idempotent creation leaves it as is, so the
/// order would never fill. A native SOL buy pays out to a wallet, and an
/// anonymous quote without a `receiver` names no account to read. The read
/// fetches the buy mint again, as the verdict cache keeps no account data and
/// the mint's extensions size the account, and the rent sysvar, as the SDK's
/// default rent is above the cluster's.
async fn buy_account_rent(
    sponsoring: Option<&Sponsoring>,
    request: &dto::Request,
    buy_program: BuyProgram,
) -> Result<u64, error::Reply> {
    if matches!(buy_program, BuyProgram::Native)
        || request.receiver.unwrap_or(request.from) == Pubkey::default()
    {
        return Ok(0);
    }
    let (Some(sponsoring), BuyProgram::Known(program)) = (sponsoring, buy_program) else {
        return Ok(max_ata_rent(&Rent::default()));
    };
    let mint = &request.buy_token;
    let [recipient, ata] = buy_token_account_candidates(request, program);
    let accounts = match sponsoring
        .rpc
        .multiple_accounts([recipient, ata, *mint, sysvar::rent::ID])
        .await
    {
        Ok(accounts) => accounts,
        Err(err) => {
            tracing::warn!(?err, "buy token account lookup failed, charging its rent");
            return Ok(max_ata_rent(&Rent::default()));
        }
    };
    if let Some(account) = accounts.get(&recipient)
        && recipient_receives(recipient, account, mint, program)?
    {
        return Ok(0);
    }
    let rent = read_ata_rent(&accounts, mint);
    match accounts.get(&ata) {
        Some(account) => ata_rent_owed(ata, account, mint, rent),
        None => Ok(rent),
    }
}

/// Whether the payout lands at `recipient`; a token account it cannot is
/// refused.
fn recipient_receives(
    recipient: Pubkey,
    account: &Account,
    mint: &Pubkey,
    program: TokenProgram,
) -> Result<bool, error::Reply> {
    if let Some(owner) = receivable_token_account_owner(account, mint) {
        // The placement proves receivability with the ATA program's idempotent
        // creation, which fails on any account but the owner's associated one.
        return if recipient == associated_token_account(&owner, mint, program) {
            Ok(true)
        } else {
            Err(invalid_buy_token_account(
                recipient,
                "is not an associated token account",
            ))
        };
    }
    // A token account of another mint, frozen, or refusing plain credits is
    // no wallet whose associated token account could take the payout.
    if TokenProgram::try_from(&account.owner).is_ok() {
        return Err(invalid_buy_token_account(
            recipient,
            "cannot receive the payout",
        ));
    }
    Ok(false)
}

fn read_ata_rent(accounts: &HashMap<Pubkey, Account>, mint: &Pubkey) -> u64 {
    let rent = accounts
        .get(&sysvar::rent::ID)
        .and_then(from_account::<Rent, _>)
        .unwrap_or_default();
    accounts
        .get(mint)
        .and_then(|mint| ata_rent(&rent, mint))
        .unwrap_or_else(|| max_ata_rent(&rent))
}

fn ata_rent_owed(
    ata: Pubkey,
    account: &Account,
    mint: &Pubkey,
    rent: u64,
) -> Result<u64, error::Reply> {
    if receivable_token_account(account, mint) {
        return Ok(0);
    }
    // Only the ATA program allocates at its address: anything there but a
    // system account's lamports, which the creation tops up to the minimum,
    // is a token account it leaves as is.
    if account.owner != solana_system_interface::program::ID || !account.data.is_empty() {
        return Err(invalid_buy_token_account(ata, "cannot receive the payout"));
    }
    Ok(rent.saturating_sub(account.lamports))
}

fn invalid_buy_token_account(account: Pubkey, reason: &str) -> error::Reply {
    error::reply(
        StatusCode::BAD_REQUEST,
        "InvalidBuyTokenAccount",
        format!("the buy token account {account} {reason}"),
    )
}

fn buy_token_account_candidates(request: &dto::Request, program: TokenProgram) -> [Pubkey; 2] {
    let recipient = request.receiver.unwrap_or(request.from);
    let ata = associated_token_account(&recipient, &request.buy_token, program);
    [recipient, ata]
}

fn associated_token_account(owner: &Pubkey, mint: &Pubkey, program: TokenProgram) -> Pubkey {
    get_associated_token_address_with_program_id(owner, mint, &program.address())
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
        solana_sdk::{account::create_account_for_test, program_pack::Pack},
        solana_testlib::{
            account_json,
            classic_mint,
            multiple_accounts_json,
            token_2022_mint,
            token_account_json,
        },
        spl_token_interface::state::{Account as TokenAccount, AccountState},
    };

    /// The mainnet rent since SIMD-0437, under the SDK's default.
    fn mainnet_rent() -> Rent {
        Rent::with_lamports_per_byte(5080)
    }

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

    fn frozen_token_account_json(mint: &Pubkey, owner: &Pubkey) -> serde_json::Value {
        account_json(&token_account(mint, owner, AccountState::Frozen))
    }

    fn token_account(mint: &Pubkey, owner: &Pubkey, state: AccountState) -> Account {
        let mut data = vec![0; TokenAccount::LEN];
        TokenAccount {
            mint: *mint,
            owner: *owner,
            state,
            ..TokenAccount::default()
        }
        .pack_into_slice(&mut data);
        Account {
            lamports: 2_039_280,
            owner: spl_token_interface::ID,
            data,
            ..Account::default()
        }
    }

    fn system_account(lamports: u64, data_len: usize) -> Account {
        Account {
            lamports,
            owner: solana_system_interface::program::ID,
            data: vec![0; data_len],
            ..Account::default()
        }
    }

    #[test]
    fn a_recipient_receives_at_its_owners_associated_token_account() {
        let (mint, owner, program) = (
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            TokenProgram::SplToken,
        );
        let ata = associated_token_account(&owner, &mint, program);
        let auxiliary = Pubkey::new_unique();
        let receives = |recipient, account: &Account| {
            recipient_receives(recipient, account, &mint, program)
                .map_err(|(_, body)| body.description.clone())
        };
        assert_eq!(
            receives(owner, &system_account(1_000_000_000, 0)),
            Ok(false)
        );
        let pda = Account {
            owner: Pubkey::new_unique(),
            ..system_account(1_000_000_000, 0)
        };
        assert_eq!(receives(owner, &pda), Ok(false));
        let initialized = token_account(&mint, &owner, AccountState::Initialized);
        assert_eq!(receives(ata, &initialized), Ok(true));
        assert_eq!(
            receives(auxiliary, &initialized),
            Err(format!(
                "the buy token account {auxiliary} is not an associated token account"
            ))
        );
        let cannot_receive = Err(format!(
            "the buy token account {ata} cannot receive the payout"
        ));
        assert_eq!(
            receives(ata, &token_account(&mint, &owner, AccountState::Frozen)),
            cannot_receive
        );
        assert_eq!(
            receives(
                ata,
                &token_account(&Pubkey::new_unique(), &owner, AccountState::Initialized)
            ),
            cannot_receive
        );
    }

    #[test]
    fn the_read_prices_the_account_by_its_mint_at_the_rent_sysvar() {
        let mint = Pubkey::new_unique();
        let rent = create_account_for_test(&mainnet_rent());
        let read =
            |entries: Vec<(Pubkey, Account)>| read_ata_rent(&entries.into_iter().collect(), &mint);
        assert_eq!(
            read(vec![
                (mint, classic_mint(6)),
                (sysvar::rent::ID, rent.clone())
            ]),
            1_488_440
        );
        assert_eq!(
            read(vec![
                (mint, token_2022_mint(&[], |_| {})),
                (sysvar::rent::ID, rent.clone()),
            ]),
            1_513_840
        );
        assert_eq!(read(vec![(mint, classic_mint(6))]), 2_039_280);
        assert_eq!(
            read(vec![(sysvar::rent::ID, rent)]),
            max_ata_rent(&mainnet_rent())
        );
    }

    #[test]
    fn the_associated_token_account_address_owes_what_it_lacks() {
        let (mint, owner) = (Pubkey::new_unique(), Pubkey::new_unique());
        let ata = associated_token_account(&owner, &mint, TokenProgram::SplToken);
        let owed = |account: &Account| {
            ata_rent_owed(ata, account, &mint, 1_488_440).map_err(|(status, _)| status)
        };
        assert_eq!(
            owed(&token_account(&mint, &owner, AccountState::Initialized)),
            Ok(0)
        );
        assert_eq!(owed(&system_account(1_000_000, 0)), Ok(488_440));
        assert_eq!(owed(&system_account(2_000_000, 0)), Ok(0));
        assert_eq!(
            owed(&system_account(1_000_000, 8)),
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            owed(&token_account(&mint, &owner, AccountState::Frozen)),
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            owed(&token_account(
                &Pubkey::new_unique(),
                &owner,
                AccountState::Initialized
            )),
            Err(StatusCode::BAD_REQUEST)
        );
    }

    /// The account read answers the recipient, its associated token account,
    /// the buy mint, then the rent sysvar.
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
        let pre_funded = account_json(&Account {
            lamports: 1_000_000,
            owner: solana_system_interface::program::ID,
            ..Default::default()
        });
        let token_account = token_account_json(&mint, &owner);
        let receiver_owner = Pubkey::new_unique();
        let receiver_ata = associated_token_account(&receiver_owner, &mint, TokenProgram::SplToken);
        let classic = account_json(&classic_mint(6));
        let token_2022 = account_json(&token_2022_mint(&[], |_| {}));
        let rent = account_json(&create_account_for_test(&mainnet_rent()));
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
                1_488_440,
            ),
            (
                None,
                [wallet.clone(), null.clone(), token_2022.clone()],
                TokenProgram::Token2022,
                1_513_840,
            ),
            (
                None,
                [wallet.clone(), pre_funded.clone(), classic.clone()],
                TokenProgram::SplToken,
                488_440,
            ),
            (
                None,
                [wallet.clone(), wallet.clone(), classic.clone()],
                TokenProgram::SplToken,
                0,
            ),
            (
                None,
                [wallet.clone(), null.clone(), null.clone()],
                TokenProgram::SplToken,
                max_ata_rent(&mainnet_rent()),
            ),
            (
                Some(receiver_ata),
                [
                    token_account_json(&mint, &receiver_owner),
                    null.clone(),
                    classic.clone(),
                ],
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
                1_488_440,
            ),
            (
                Some(receiver),
                [null.clone(), null.clone(), classic.clone()],
                TokenProgram::SplToken,
                1_488_440,
            ),
        ] {
            let sponsoring = sponsoring(multiple_accounts_json(
                accounts.iter().cloned().chain([rent.clone()]),
            ));
            assert_eq!(
                buy_account_rent(
                    Some(&sponsoring),
                    &request(owner, mint, receiver),
                    BuyProgram::Known(program)
                )
                .await
                .unwrap(),
                cost,
                "{receiver:?} {accounts:?} {program:?}"
            );
        }
        let without_rent_sysvar = sponsoring(multiple_accounts_json([wallet, null, classic]));
        assert_eq!(
            buy_account_rent(
                Some(&without_rent_sysvar),
                &request(owner, mint, None),
                BuyProgram::Known(TokenProgram::SplToken)
            )
            .await
            .unwrap(),
            2_039_280
        );
    }

    /// A receiving token account away from its owner's associated address is
    /// refused too: the placement cannot create it idempotently.
    #[tokio::test]
    async fn an_unreceivable_buy_token_account_is_refused() {
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
        let frozen = frozen_token_account_json(&mint, &owner);
        let auxiliary = token_account_json(&mint, &owner);
        let other_mint_account = token_account_json(&Pubkey::new_unique(), &owner);
        let system_account_with_data = account_json(&Account {
            lamports: 1_000_000_000,
            owner: solana_system_interface::program::ID,
            data: vec![0; 8],
            ..Default::default()
        });
        let classic = account_json(&classic_mint(6));
        let null = serde_json::Value::Null;
        for (receiver, accounts) in [
            (None, [wallet.clone(), frozen.clone(), classic.clone()]),
            (
                None,
                [wallet.clone(), other_mint_account.clone(), classic.clone()],
            ),
            (
                None,
                [wallet.clone(), system_account_with_data, classic.clone()],
            ),
            (Some(receiver), [frozen, null.clone(), classic.clone()]),
            (
                Some(receiver),
                [other_mint_account, null.clone(), classic.clone()],
            ),
            (Some(receiver), [auxiliary, null, classic]),
        ] {
            let sponsoring = sponsoring(multiple_accounts_json(accounts.clone()));
            let (status, body) = buy_account_rent(
                Some(&sponsoring),
                &request(owner, mint, receiver),
                BuyProgram::Known(TokenProgram::SplToken),
            )
            .await
            .unwrap_err();
            assert_eq!(status, StatusCode::BAD_REQUEST, "{receiver:?} {accounts:?}");
            assert_eq!(body.error_type, "InvalidBuyTokenAccount");
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
                buy_account_rent(None, &request, BuyProgram::new(&request, None))
                    .await
                    .unwrap(),
                0,
                "{request:?}"
            );
        }
    }

    /// Even an SPL Token mint is priced at the Token-2022 ceiling unread, and
    /// an anonymous quote naming a `receiver` has an account to price.
    #[tokio::test]
    async fn an_unread_buy_token_account_owes_the_largest_rent() {
        let largest = max_ata_rent(&Rent::default());
        let anonymous = request(
            Pubkey::default(),
            Pubkey::new_unique(),
            Some(Pubkey::new_unique()),
        );
        assert_eq!(
            buy_account_rent(None, &anonymous, BuyProgram::Unread)
                .await
                .unwrap(),
            largest
        );
        let request = request(Pubkey::new_unique(), Pubkey::new_unique(), None);
        let exists = sponsoring(multiple_accounts_json([
            serde_json::Value::Null,
            token_account_json(&request.buy_token, &request.from),
            account_json(&classic_mint(6)),
            account_json(&create_account_for_test(&mainnet_rent())),
        ]));
        let failing = sponsoring(serde_json::json!("not an account list"));
        for (sponsoring, program) in [
            (None, BuyProgram::Known(TokenProgram::SplToken)),
            (Some(&exists), BuyProgram::Unread),
            (Some(&failing), BuyProgram::Known(TokenProgram::SplToken)),
        ] {
            assert_eq!(
                buy_account_rent(sponsoring, &request, program)
                    .await
                    .unwrap(),
                largest
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
