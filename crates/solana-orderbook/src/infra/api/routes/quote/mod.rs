//! The quote endpoint: what one order would trade for right now.

pub mod dto;

use {
    super::mint::{ensure_settleable, token_mints},
    crate::infra::{
        api::{Sponsoring, State, ValidationParameters, error, extract},
        db,
        prices,
        quoter,
    },
    app_data::OrderCreationAppData,
    axum::{Json, http::StatusCode},
    chain_types::{ChainTypes, solana::Solana},
    chrono::Utc,
    cow_settlement_interface::{
        data::intent::ENCODED_NATIVE_SOL_TRANSFER,
        token_program::TokenProgram,
    },
    database::{byte_array::ByteArray, solana::OrderKind},
    serde::Serialize,
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::{
        account::{Account, from_account},
        pubkey::Pubkey,
        rent::Rent,
        sysvar,
    },
    solana_token::{ata_rent, max_ata_rent, receivable_token_account},
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id,
    spl_token_interface::native_mint,
    std::{cmp::Reverse, collections::HashMap, num::NonZero, time::Duration},
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
    let stored = match &request.app_data {
        OrderCreationAppData::Hash { hash } => super::app_data::find(state.pool(), *hash)
            .await
            .map_err(|err| {
                tracing::error!(?err, "app data lookup failed");
                error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
            })?,
        _ => None,
    };
    super::app_data::resolve(
        &request.app_data,
        stored.as_deref(),
        None,
        state.app_data_size_limit(),
    )?;
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
    let quotes = async {
        // Every driver failure answers as no liquidity, the EVM mapping for
        // estimator errors.
        state
            .quoter()
            .quotes(&order)
            .await
            .map_err(|quoter::Error::NoQuotes| {
                error::reply(StatusCode::NOT_FOUND, "NoLiquidity", "no route found")
            })
    };
    // The autopilot leaves an order on a token no estimator prices out of
    // every auction, so the quote fails before such an order is placed.
    let prices = state.prices();
    let price = |token| async move {
        prices
            .price(token)
            .await
            .map_err(|err| price_error(token, err))
    };
    let (quotes, buy_token_ata_costs, sell_price, buy_price) = futures::try_join!(
        quotes,
        buy_account_rent(state.sponsoring(), &request, buy_program),
        price(request.sell_token),
        price(request.buy_token),
    )?;
    // The cost comes out of the side the trader does not fix, the side the
    // driver cuts at settlement.
    let (token, price) = match kind {
        dto::Kind::Sell => (request.buy_token, buy_price),
        dto::Kind::Buy => (request.sell_token, sell_price),
    };
    let price =
        NonZero::new(price).ok_or_else(|| price_error(token, prices::Error::NoLiquidity))?;
    let ranked = rank(quotes, kind, buy_token_ata_costs, price);
    // Before taking the winner, so a competition without one is logged too.
    let competition = competition(&request, kind, amount, buy_token_ata_costs, &ranked);
    match serde_json::to_string(&competition) {
        Ok(json) => tracing::debug!(competition = %json, "computed quote"),
        Err(err) => tracing::warn!(?err, "failed to serialize the quote competition"),
    }
    let (quoted, costs) = ranked
        .into_iter()
        .find_map(|(_, netted)| netted)
        .ok_or_else(|| {
            error::reply(
                StatusCode::BAD_REQUEST,
                "SellAmountDoesNotCoverFee",
                "the quoted amount does not cover the execution cost",
            )
        })?;
    quoted.count_win(order.kind);
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
            // A document answers with its hash alongside.
            app_data: match &request.app_data {
                OrderCreationAppData::Full { full } => OrderCreationAppData::Both {
                    full: full.clone(),
                    expected: request.app_data.hash(),
                },
                app_data => app_data.clone(),
            },
            fee_amount: 0,
            costs: dto::Costs {
                token,
                network_fees: costs.network_fees,
                buy_token_ata_costs: costs.buy_token_ata_costs,
                protocol_fee: 0,
                total: costs.total,
            },
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

/// Every quote paired with its [`net`] self, best netted first. Quotes that
/// cannot absorb their cost rank last, so the first netted one is the winner.
fn rank(
    quotes: Vec<quoter::Quote>,
    kind: dto::Kind,
    rent: u64,
    price: NonZero<u64>,
) -> Vec<(quoter::Quote, Option<(quoter::Quote, Costs)>)> {
    let mut ranked: Vec<_> = quotes
        .into_iter()
        .map(|quote| {
            let netted = net(quote.clone(), kind, rent, price);
            (quote, netted)
        })
        .collect();
    match kind {
        dto::Kind::Sell => ranked.sort_by_key(|(_, netted)| {
            (
                netted.is_none(),
                netted.as_ref().map(|(q, _)| Reverse(q.buy_amount)),
            )
        }),
        dto::Kind::Buy => ranked
            .sort_by_key(|(_, netted)| (netted.is_none(), netted.as_ref().map(|(q, _)| q.sell_amount))),
    }
    ranked
}

/// The `computed quote` log line: every driver's answer, best first.
#[derive(Serialize)]
struct Competition<'a> {
    request: CompetitionRequest,
    quotes: Vec<CompetitionQuote<'a>>,
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompetitionRequest {
    #[serde_as(as = "DisplayFromStr")]
    sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    buy_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    amount: u64,
    kind: dto::Kind,
    #[serde_as(as = "DisplayFromStr")]
    buy_token_ata_costs: u64,
}

/// One driver's answer: `sell_amount` and `buy_amount` are after the execution
/// cost, `quoted_*` as the driver returned them.
#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompetitionQuote<'a> {
    driver: &'a str,
    #[serde_as(as = "DisplayFromStr")]
    solver: Pubkey,
    /// `None` when the quote cannot absorb its execution cost.
    #[serde_as(as = "Option<DisplayFromStr>")]
    sell_amount: Option<u64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    buy_amount: Option<u64>,
    #[serde_as(as = "DisplayFromStr")]
    quoted_sell_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    quoted_buy_amount: u64,
    /// In lamports.
    #[serde_as(as = "DisplayFromStr")]
    network_fee: u64,
    elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

fn competition<'a>(
    request: &dto::Request,
    kind: dto::Kind,
    amount: u64,
    rent: u64,
    ranked: &'a [(quoter::Quote, Option<(quoter::Quote, Costs)>)],
) -> Competition<'a> {
    Competition {
        request: CompetitionRequest {
            sell_token: request.sell_token,
            buy_token: request.buy_token,
            amount,
            kind,
            buy_token_ata_costs: rent,
        },
        quotes: ranked
            .iter()
            .map(|(quote, netted)| CompetitionQuote {
                driver: &quote.driver,
                solver: quote.solver,
                sell_amount: netted.as_ref().map(|(q, _)| q.sell_amount),
                buy_amount: netted.as_ref().map(|(q, _)| q.buy_amount),
                quoted_sell_amount: quote.sell_amount,
                quoted_buy_amount: quote.buy_amount,
                network_fee: quote.network_fee,
                elapsed_ms: quote.elapsed.as_millis().try_into().unwrap_or(u64::MAX),
                error: netted
                    .is_none()
                    .then_some("the quoted amount does not cover the execution cost"),
            })
            .collect(),
    }
}

/// A quote's execution cost in the token it comes out of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Costs {
    network_fees: u64,
    buy_token_ata_costs: u64,
    /// The sum, what the quote is cut by.
    total: u64,
}

/// `quote` with its execution cost, `rent` plus its own network fee, taken
/// out of the side the trader does not fix: a sell order delivers less buy
/// token, a buy order pulls more sell token. Each cost converts at `price` on
/// its own, rounded up against the trader. `None` when the amount cannot
/// absorb the cost.
fn net(
    quote: quoter::Quote,
    kind: dto::Kind,
    rent: u64,
    price: NonZero<u64>,
) -> Option<(quoter::Quote, Costs)> {
    let network_fees = in_token(quote.network_fee, price)?;
    let buy_token_ata_costs = in_token(rent, price)?;
    let total = network_fees.checked_add(buy_token_ata_costs)?;
    let quote = match kind {
        dto::Kind::Sell => quoter::Quote {
            buy_amount: quote
                .buy_amount
                .checked_sub(total)
                .filter(|delivered| *delivered > 0)?,
            ..quote
        },
        dto::Kind::Buy => quoter::Quote {
            sell_amount: quote.sell_amount.checked_add(total)?,
            ..quote
        },
    };
    Some((
        quote,
        Costs {
            network_fees,
            buy_token_ata_costs,
            total,
        },
    ))
}

/// What `lamports` are worth in the token priced at `price`, rounded up.
/// `None` past `u64::MAX`.
fn in_token(lamports: u64, price: NonZero<u64>) -> Option<u64> {
    let atoms = (u128::from(lamports) * u128::from(Solana::NATIVE_PRICE_DENOMINATOR))
        .div_ceil(u128::from(price.get()));
    u64::try_from(atoms).ok()
}

/// An unpriced or rate-limited token is no liquidity, anything else is the
/// autopilot misbehaving.
fn price_error(token: Pubkey, err: prices::Error) -> error::Reply {
    match err {
        prices::Error::NoLiquidity | prices::Error::RateLimited => error::reply(
            StatusCode::NOT_FOUND,
            "NoLiquidity",
            format!("no native price for {token}"),
        ),
        prices::Error::Internal(err) => {
            tracing::error!(?err, %token, "native price lookup failed");
            error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
        }
    }
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
/// order would never fill. A native SOL buy pays out to a wallet. An
/// anonymous quote without a `receiver` names no account to read and is
/// priced as if it were missing, as an unverified EVM quote keeps the full
/// fee. The read fetches the buy mint again, as the verdict cache keeps no
/// account data and the mint's extensions size the account, and the rent
/// sysvar, as the SDK's default rent is above the cluster's.
async fn buy_account_rent(
    sponsoring: Option<&Sponsoring>,
    request: &dto::Request,
    buy_program: BuyProgram,
) -> Result<u64, error::Reply> {
    if matches!(buy_program, BuyProgram::Native) {
        return Ok(0);
    }
    let (Some(sponsoring), BuyProgram::Known(program)) = (sponsoring, buy_program) else {
        return Ok(max_ata_rent(&Rent::default()));
    };
    let mint = &request.buy_token;
    let [recipient, ata] = buy_token_account_candidates(request, program);
    let addresses = if recipient == Pubkey::default() {
        vec![*mint, sysvar::rent::ID]
    } else {
        vec![recipient, ata, *mint, sysvar::rent::ID]
    };
    let accounts = match sponsoring.rpc.multiple_accounts(addresses).await {
        Ok(accounts) => accounts,
        Err(err) => {
            tracing::warn!(?err, "buy token account lookup failed, charging its rent");
            return Ok(max_ata_rent(&Rent::default()));
        }
    };
    if let Some(account) = accounts.get(&recipient)
        && recipient_receives(recipient, account, mint)?
    {
        return Ok(0);
    }
    let rent = read_ata_rent(&accounts, mint);
    match accounts.get(&ata) {
        Some(account) => ata_rent_owed(ata, account, mint, rent),
        None => Ok(rent),
    }
}

/// Whether the payout lands at `recipient`: any token account the settlement's
/// plain transfer of `mint` can credit, whoever owns it. One it cannot is
/// refused.
fn recipient_receives(
    recipient: Pubkey,
    account: &Account,
    mint: &Pubkey,
) -> Result<bool, error::Reply> {
    if receivable_token_account(account, mint) {
        return Ok(true);
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
            app_data: Default::default(),
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

    fn quote(sell_amount: u64, buy_amount: u64, network_fee: u64) -> quoter::Quote {
        quoter::Quote {
            driver: "http://driver/".to_owned(),
            elapsed: Duration::from_millis(1),
            sell_amount,
            buy_amount,
            solver: Pubkey::default(),
            network_fee,
        }
    }

    fn price(native_price: u64) -> NonZero<u64> {
        NonZero::new(native_price).unwrap()
    }

    fn best(
        quotes: Vec<quoter::Quote>,
        kind: dto::Kind,
        rent: u64,
        price: NonZero<u64>,
    ) -> Option<(quoter::Quote, Costs)> {
        rank(quotes, kind, rent, price)
            .into_iter()
            .find_map(|(_, netted)| netted)
    }

    fn legs(quote: &quoter::Quote) -> (u64, u64) {
        (quote.sell_amount, quote.buy_amount)
    }

    /// 600 lamports of rent and a 400 lamport fee are 2000 atoms of a token
    /// worth half a lamport an atom: off the buy leg of a sell order, onto
    /// the sell leg of a buy order. Nothing to take leaves the quote as the
    /// driver gave it.
    #[test]
    fn the_cost_comes_out_of_the_side_the_trader_does_not_fix() {
        let price = price(Solana::NATIVE_PRICE_DENOMINATOR / 2);
        let (sell, costs) = net(quote(1_000, 5_000, 400), dto::Kind::Sell, 600, price).unwrap();
        assert_eq!(legs(&sell), (1_000, 3_000));
        assert_eq!(
            costs,
            Costs {
                network_fees: 800,
                buy_token_ata_costs: 1_200,
                total: 2_000,
            }
        );
        let (buy, _) = net(quote(1_000, 5_000, 400), dto::Kind::Buy, 600, price).unwrap();
        assert_eq!(legs(&buy), (3_000, 5_000));
        let (free, _) = net(quote(1_000, 5_000, 0), dto::Kind::Sell, 0, price).unwrap();
        assert_eq!(legs(&free), (1_000, 5_000));
    }

    /// At three lamports an atom, one lamport is a third of an atom and
    /// costs a whole one, three lamports cost one, four cost two. The rent
    /// and the fee round up on their own, so a lamport of each costs two.
    #[test]
    fn the_cut_rounds_up_against_the_trader() {
        let price = price(3 * Solana::NATIVE_PRICE_DENOMINATOR);
        for (lamports, buy_amount) in [(1, 4_999), (3, 4_999), (4, 4_998)] {
            let (sell, _) = net(quote(1_000, 5_000, lamports), dto::Kind::Sell, 0, price).unwrap();
            assert_eq!(sell.buy_amount, buy_amount);
            let (buy, _) = net(quote(1_000, 5_000, lamports), dto::Kind::Buy, 0, price).unwrap();
            assert_eq!(buy.sell_amount, 1_000 + 5_000 - buy_amount);
        }
        let (sell, costs) = net(quote(1_000, 5_000, 1), dto::Kind::Sell, 1, price).unwrap();
        assert_eq!((sell.buy_amount, costs.total), (4_998, 2));
    }

    /// A sell order delivering nothing after the cut, a buy order whose sell
    /// leg overflows, and a cut past `u64::MAX` are all dropped.
    #[test]
    fn a_quote_that_cannot_absorb_its_cost_is_dropped() {
        let lamport_an_atom = price(Solana::NATIVE_PRICE_DENOMINATOR);
        let sell = |buy_amount, fee| {
            net(
                quote(1_000, buy_amount, fee),
                dto::Kind::Sell,
                0,
                lamport_an_atom,
            )
        };
        assert!(sell(500, 500).is_none());
        assert_eq!(sell(501, 500).unwrap().0.buy_amount, 1);
        assert!(net(quote(u64::MAX, 500, 1), dto::Kind::Buy, 0, lamport_an_atom).is_none());
        assert!(net(quote(1_000, 5_000, u64::MAX), dto::Kind::Sell, 0, price(1)).is_none());
    }

    /// The raw-best quote loses to the one whose own network fee leaves the
    /// trader more, and a quote that cannot absorb its cost never wins.
    #[test]
    fn the_best_quote_is_the_best_netted_one() {
        let lamport_an_atom = price(Solana::NATIVE_PRICE_DENOMINATOR);
        let quotes = vec![
            quote(1_000, 5_000, 0),
            quote(1_000, 5_100, 200),
            quote(1_000, 10, 0),
        ];
        let (sell, _) = best(quotes, dto::Kind::Sell, 50, lamport_an_atom).unwrap();
        assert_eq!((sell.buy_amount, sell.network_fee), (4_950, 0));
        let quotes = vec![quote(1_000, 5_000, 0), quote(900, 5_000, 200)];
        let (buy, _) = best(quotes, dto::Kind::Buy, 50, lamport_an_atom).unwrap();
        assert_eq!((buy.sell_amount, buy.network_fee), (1_050, 0));
        assert!(
            best(
                vec![quote(1_000, 10, 0)],
                dto::Kind::Sell,
                50,
                lamport_an_atom
            )
            .is_none()
        );
    }

    #[test]
    fn the_competition_keeps_every_quote_best_first() {
        let lamport_an_atom = price(Solana::NATIVE_PRICE_DENOMINATOR);
        let named = |driver: &str, buy_amount, fee| quoter::Quote {
            driver: driver.to_owned(),
            ..quote(1_000, buy_amount, fee)
        };
        let ranked = rank(
            vec![
                named("raw-best", 5_100, 200),
                named("dropped", 10, 0),
                named("best", 5_000, 0),
            ],
            dto::Kind::Sell,
            50,
            lamport_an_atom,
        );
        let order = ranked
            .iter()
            .map(|(quote, netted)| (&*quote.driver, netted.as_ref().map(|(q, _)| q.buy_amount)))
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            [
                ("best", Some(4_950)),
                ("raw-best", Some(4_850)),
                ("dropped", None)
            ]
        );
    }

    /// Base58 pubkeys and amounts as strings, like the API.
    #[test]
    fn the_competition_serializes_for_the_log_line() {
        let ranked = rank(
            vec![quote(1_000, 5_000, 400), quote(1_000, 10, 0)],
            dto::Kind::Sell,
            100,
            price(Solana::NATIVE_PRICE_DENOMINATOR),
        );
        let request = request(Pubkey::new_unique(), Pubkey::new_unique(), None);
        let json =
            serde_json::to_value(competition(&request, dto::Kind::Sell, 1_000, 100, &ranked))
                .unwrap();
        assert_eq!(json["request"]["kind"], "sell");
        assert_eq!(json["request"]["amount"], "1000");
        assert_eq!(json["request"]["buyTokenAtaCosts"], "100");
        assert_eq!(json["request"]["sellToken"], request.sell_token.to_string());
        let winner = &json["quotes"][0];
        assert_eq!(winner["driver"], "http://driver/");
        assert_eq!(winner["solver"], Pubkey::default().to_string());
        assert_eq!(winner["buyAmount"], "4500");
        assert_eq!(winner["quotedBuyAmount"], "5000");
        assert_eq!(winner["networkFee"], "400");
        assert_eq!(winner["elapsedMs"], 1);
        assert!(winner.get("error").is_none());
        let dropped = &json["quotes"][1];
        assert!(dropped["buyAmount"].is_null());
        assert_eq!(
            dropped["error"],
            "the quoted amount does not cover the execution cost"
        );
    }

    #[test]
    fn a_recipient_receives_at_any_token_account_of_the_mint() {
        let (mint, owner) = (Pubkey::new_unique(), Pubkey::new_unique());
        let ata = associated_token_account(&owner, &mint, TokenProgram::SplToken);
        let auxiliary = Pubkey::new_unique();
        let receives = |recipient, account: &Account| {
            recipient_receives(recipient, account, &mint)
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
        assert_eq!(receives(auxiliary, &initialized), Ok(true));
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
                Some(receiver),
                [
                    token_account_json(&mint, &Pubkey::new_unique()),
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
            (Some(receiver), [other_mint_account, null, classic]),
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

    /// The account read answers the buy mint, then the rent sysvar: an
    /// anonymous quote has no recipient to read.
    #[tokio::test]
    async fn an_anonymous_quote_owes_the_rent_of_a_missing_account() {
        let request = request(Pubkey::default(), Pubkey::new_unique(), None);
        let sponsoring = sponsoring(multiple_accounts_json([
            account_json(&classic_mint(6)),
            account_json(&create_account_for_test(&mainnet_rent())),
        ]));
        assert_eq!(
            buy_account_rent(
                Some(&sponsoring),
                &request,
                BuyProgram::Known(TokenProgram::SplToken)
            )
            .await
            .unwrap(),
            1_488_440
        );
    }

    #[tokio::test]
    async fn native_buys_owe_no_rent() {
        let owner = Pubkey::new_unique();
        for request in [
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

    /// Even an SPL Token mint is priced at the Token-2022 ceiling unread,
    /// with or without a recipient.
    #[tokio::test]
    async fn an_unread_buy_token_account_owes_the_largest_rent() {
        let largest = max_ata_rent(&Rent::default());
        let anonymous = request(Pubkey::default(), Pubkey::new_unique(), None);
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
