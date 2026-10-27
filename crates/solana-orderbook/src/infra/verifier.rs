//! Quote verification: each driver's quoted settlement is simulated after a
//! transaction that creates and funds the quoted order.

use {
    super::quoter::{Kind, Order, Quote},
    configs::price_estimation::QuoteVerificationMode,
    cow_settlement_client::instruction::CreateOrder,
    cow_settlement_interface::{
        data::intent::{ENCODED_NATIVE_SOL_TRANSFER, OrderIntent, OrderKind},
        pda::{
            buffer::{find_buffer_pda, find_native_sol_buffer_pda},
            state::find_state_pda,
        },
        token_program::TokenProgram,
    },
    cow_solana_rpc::SolanaRPC,
    futures::future::join_all,
    moka::sync::Cache,
    solana_sdk::{
        account::Account,
        instruction::Instruction,
        message::{Message, VersionedMessage},
        program_pack::Pack,
        pubkey::Pubkey,
        signature::Signature,
        transaction::VersionedTransaction,
    },
    solana_token::quote::QuoteOrder,
    spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent,
    spl_token_2022_interface::{
        extension::StateWithExtensions,
        instruction::{approve, mint_to, transfer_checked},
        state::{Account as TokenAccount, AccountState, Mint, Multisig},
    },
    std::time::{Duration, Instant},
};

/// How long a pair's funding is reused: its mints, the sell mint's authority
/// and its holders' balances. A failed creation leg drops it early.
const FUNDING_TTL: Duration = Duration::from_secs(2 * 60);

/// How long a mint's largest holder accounts are kept.
const HOLDERS_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// The least time between two lookups of a mint's largest holders. Holders
/// that fund no quoted amount are looked up again once it passed.
const HOLDERS_REFRESH: Duration = Duration::from_secs(10 * 60);

/// How many entries each cache keeps.
const CACHE_CAPACITY: u64 = 10_000;

/// Settles quotes in simulation.
pub struct Verifier {
    /// Reads the mints and the accounts that fund them.
    rpc: SolanaRPC,
    /// Simulates the bundle, a node with the Jito `simulateBundle` extension.
    bundle: SolanaRPC,
    /// Lists a mint's largest holders, a node with the account index. `None`
    /// leaves a mint nobody can mint unfunded, its quotes unverified.
    holders: Option<SolanaRPC>,
    program_id: Pubkey,
    /// The share of the sell or buy amount the buffers may pay before the
    /// quote is dropped.
    inaccuracy_limit: f64,
    mode: QuoteVerificationMode,
    /// Each pair's funding, keyed by sell and buy mint.
    fundings: Cache<(Pubkey, Pubkey), Funding>,
    /// Each mint's largest token accounts as the holders node listed them.
    holder_accounts: Cache<Pubkey, HolderAccounts>,
}

/// A mint's largest token accounts, largest first, and when they were listed.
#[derive(Clone, Debug)]
struct HolderAccounts {
    accounts: Vec<Pubkey>,
    listed_at: Instant,
}

/// A token account of a mint that can give its tokens away in simulation.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Holder {
    account: Pubkey,
    owner: Pubkey,
    amount: u64,
}

/// What a quote's verification found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// The order receives the quoted amount, and the buffers pay less than
    /// the inaccuracy limit.
    Verified,
    /// The simulation could not confirm the quote, which stays as the driver
    /// gave it.
    Unverified,
    /// The buffers pay the inaccuracy limit or more. The quote is dropped.
    Inaccurate,
}

impl Verdict {
    fn label(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
            Self::Inaccurate => "inaccurate",
        }
    }
}

/// What funds a simulated sell of a mint, and the programs the quote's
/// accounts live under.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Funding {
    sell_program: TokenProgram,
    decimals: u8,
    /// `None` for a native SOL buy.
    buy_program: Option<TokenProgram>,
    source: Source,
}

/// Who supplies the sell tokens in simulation. Signatures go unchecked, so
/// anyone who can move them on chain can.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    /// The mint authority mints them. `signers` is empty when the authority
    /// signs itself, or the threshold's worth of its multisig's signers.
    Authority {
        authority: Pubkey,
        signers: Vec<Pubkey>,
    },
    /// The largest holders transfer them, the first holding the amount.
    Holders(Vec<Holder>),
}

impl Funding {
    /// Whether the source can fund a sell of `amount`.
    fn covers(&self, amount: u64) -> bool {
        match &self.source {
            Source::Authority { .. } => true,
            Source::Holders(holders) => holders.iter().any(|holder| holder.amount >= amount),
        }
    }

    /// The instruction filling `order`'s sell token account with its sell
    /// amount. `None` when no holder holds that much.
    fn instruction(&self, order: &QuoteOrder) -> Option<Instruction> {
        let token_program = self.sell_program.address();
        let destination = order.sell_token_account();
        let instruction = match &self.source {
            Source::Authority { authority, signers } => mint_to(
                &token_program,
                &order.sell_mint,
                &destination,
                authority,
                &signers.iter().collect::<Vec<_>>(),
                order.sell_amount,
            ),
            Source::Holders(holders) => {
                let holder = holders
                    .iter()
                    .find(|holder| holder.amount >= order.sell_amount)?;
                transfer_checked(
                    &token_program,
                    &holder.account,
                    &order.sell_mint,
                    &destination,
                    &holder.owner,
                    &[],
                    order.sell_amount,
                    self.decimals,
                )
            }
        };
        Some(instruction.expect("a token program id passes the program check"))
    }
}

/// A mint as the verification reads it.
struct MintInfo {
    program: TokenProgram,
    decimals: u8,
    /// `None` once revoked.
    authority: Option<Pubkey>,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Rpc(#[from] cow_solana_rpc::Error),
    #[error("{0} is not a mint")]
    NotAMint(Pubkey),
}

/// What an account's balance counts in.
#[derive(Clone, Copy, Debug)]
enum Unit {
    Lamports,
    Tokens,
}

impl Verifier {
    pub fn new(
        rpc: SolanaRPC,
        bundle: SolanaRPC,
        holders: Option<SolanaRPC>,
        program_id: Pubkey,
        inaccuracy_limit: f64,
        mode: QuoteVerificationMode,
    ) -> Self {
        Self {
            rpc,
            bundle,
            holders,
            program_id,
            inaccuracy_limit,
            mode,
            fundings: Cache::builder()
                .time_to_live(FUNDING_TTL)
                .max_capacity(CACHE_CAPACITY)
                .build(),
            holder_accounts: Cache::builder()
                .time_to_live(HOLDERS_TTL)
                .max_capacity(CACHE_CAPACITY)
                .build(),
        }
    }

    /// Whether a verified quote outranks an unverified one.
    pub fn prefers_verified(&self) -> bool {
        matches!(self.mode, QuoteVerificationMode::Prefer)
    }

    /// Verify each of the `quotes` of `order`, whose quoted order `owner`
    /// owns: a verified quote is marked, an inaccurate one dropped, the rest
    /// stay as the drivers gave them.
    pub async fn verify(&self, order: &Order, owner: Pubkey, quotes: Vec<Quote>) -> Vec<Quote> {
        let largest_sell = quotes
            .iter()
            .map(|quote| quote.sell_amount)
            .max()
            .unwrap_or(0);
        let funding = self
            .funding(order, largest_sell)
            .await
            .inspect_err(|err| tracing::debug!(?err, "quote funding not read"))
            .ok();
        let verdicts = join_all(quotes.into_iter().map(|quote| async {
            let verdict = match &funding {
                Some(funding) => self.verify_one(order, owner, funding, &quote).await,
                None => Verdict::Unverified,
            };
            metrics()
                .quote_verifications
                .with_label_values(&[verdict.label(), quote.driver.as_str()])
                .inc();
            (quote, verdict)
        }))
        .await;
        verdicts
            .into_iter()
            .filter_map(|(mut quote, verdict)| match verdict {
                Verdict::Verified => {
                    quote.verified = true;
                    Some(quote)
                }
                Verdict::Unverified => Some(quote),
                Verdict::Inaccurate => None,
            })
            .collect()
    }

    /// Settle `quote`'s transaction behind the creation of the quoted order
    /// and read what the fill did to the order's buy account and to the
    /// settlement program's buffers.
    async fn verify_one(
        &self,
        order: &Order,
        owner: Pubkey,
        funding: &Funding,
        quote: &Quote,
    ) -> Verdict {
        let driver = quote.driver.as_str();
        let Some(settlement) = &quote.transaction else {
            return Verdict::Unverified;
        };
        // The simulation checks no signature, so a transaction may only sign
        // as the solver, which signs for real on chain.
        if signers(settlement) != [quote.solver] {
            tracing::debug!(
                driver,
                "quote unverified, its settlement signs as more than the solver"
            );
            return Verdict::Unverified;
        }
        let quoted = QuoteOrder {
            owner,
            sell_mint: order.sell_token,
            sell_program: funding.sell_program,
            buy_mint: order.buy_token,
            buy_program: funding.buy_program,
            sell_amount: quote.sell_amount,
            buy_amount: quote.buy_amount,
            kind: match order.kind {
                Kind::Sell => OrderKind::Sell,
                Kind::Buy => OrderKind::Buy,
            },
        };
        let Some(intent) = quoted.intent() else {
            return Verdict::Unverified;
        };
        let Some(fund) = funding.instruction(&quoted) else {
            tracing::debug!(driver, "quote unverified, nothing funds its sell");
            return Verdict::Unverified;
        };
        let creation = creation_leg(&self.program_id, &quote.solver, &quoted, &intent, fund);
        let buy_unit = match funding.buy_program {
            Some(_) => Unit::Tokens,
            None => Unit::Lamports,
        };
        let buy_buffer = match funding.buy_program {
            Some(_) => find_buffer_pda(&self.program_id, &order.buy_token).0,
            None => find_native_sol_buffer_pda(&self.program_id).0,
        };
        let watched = [
            (quoted.buy_account(), buy_unit),
            (
                find_buffer_pda(&self.program_id, &order.sell_token).0,
                Unit::Tokens,
            ),
            (buy_buffer, buy_unit),
        ];
        let bundle = [creation, settlement.clone()];
        let accounts = [
            Vec::new(),
            watched.iter().map(|(account, _)| *account).collect(),
        ];
        let response = match self.bundle.simulate_bundle(&bundle, &accounts).await {
            Ok(response) => response,
            Err(err) => {
                tracing::debug!(
                    driver,
                    ?err,
                    "quote unverified, the bundle simulation failed"
                );
                return Verdict::Unverified;
            }
        };
        let results = response.value;
        if let Some(err) = results.first().and_then(|result| result.err.as_ref()) {
            tracing::debug!(
                driver,
                ?err,
                logs = ?results[0].logs,
                "quote unverified, the creation leg fails in simulation"
            );
            self.fundings
                .invalidate(&(order.sell_token, order.buy_token));
            return Verdict::Unverified;
        }
        let Some(settled) = results.get(1) else {
            tracing::debug!(
                driver,
                "quote unverified, the bundle simulation stopped short"
            );
            return Verdict::Unverified;
        };
        if let Some(err) = &settled.err {
            tracing::debug!(
                driver,
                ?err,
                logs = ?settled.logs,
                "quote unverified, its settlement fails in simulation"
            );
            return Verdict::Unverified;
        }
        let balances = |accounts: &[Account]| -> Option<[u64; 3]> {
            Some([
                balance(accounts.first()?, watched[0].1),
                balance(accounts.get(1)?, watched[1].1),
                balance(accounts.get(2)?, watched[2].1),
            ])
        };
        let (Some(pre), Some(post)) = (
            balances(&settled.pre_execution_accounts),
            balances(&settled.post_execution_accounts),
        ) else {
            tracing::debug!(
                driver,
                "quote unverified, the simulation reported no accounts"
            );
            return Verdict::Unverified;
        };
        let received = post[0].saturating_sub(pre[0]);
        if received < quote.buy_amount {
            tracing::debug!(
                driver,
                received,
                quoted = quote.buy_amount,
                "quote unverified, its settlement delivers less than quoted"
            );
            return Verdict::Unverified;
        }
        // The EVM check: the buffers may pay less than the limit's share of
        // either amount.
        for (index, amount, side) in [(1, quote.sell_amount, "sell"), (2, quote.buy_amount, "buy")]
        {
            let lost = pre[index].saturating_sub(post[index]);
            if lost as f64 >= self.inaccuracy_limit * amount as f64 {
                tracing::debug!(
                    driver,
                    side,
                    lost,
                    amount,
                    "quote dropped, the buffers pay for it"
                );
                return Verdict::Inaccurate;
            }
        }
        tracing::debug!(
            driver,
            slot = response.context.slot,
            units_consumed = ?settled.units_consumed,
            "quote verified"
        );
        Verdict::Verified
    }

    /// What funds a simulated sell of the order's sell mint: the pair's cached
    /// read, unless its holders fund no sell of `needed` and their lookup is
    /// due.
    async fn funding(&self, order: &Order, needed: u64) -> Result<Funding, Error> {
        let key = (order.sell_token, order.buy_token);
        if let Some(funding) = self.fundings.get(&key)
            && (funding.covers(needed) || !self.holders_due(&order.sell_token))
        {
            return Ok(funding);
        }
        let funding = self.read_funding(order, needed).await?;
        self.fundings.insert(key, funding.clone());
        Ok(funding)
    }

    /// What funds a simulated sell of the order's sell mint, read from the
    /// chain: its authority, or its largest holders once the authority is
    /// revoked.
    async fn read_funding(&self, order: &Order, needed: u64) -> Result<Funding, Error> {
        let buy_mint = (order.buy_token != ENCODED_NATIVE_SOL_TRANSFER).then_some(order.buy_token);
        let accounts = self
            .rpc
            .multiple_accounts([order.sell_token].into_iter().chain(buy_mint))
            .await?;
        let read = |mint: Pubkey| mint_info(accounts.get(&mint)).ok_or(Error::NotAMint(mint));
        let sell = read(order.sell_token)?;
        let buy_program = buy_mint.map(read).transpose()?.map(|info| info.program);
        let source = match sell.authority {
            Some(authority) => Source::Authority {
                authority,
                signers: self.multisig_signers(authority).await?,
            },
            None => Source::Holders(self.holders(order.sell_token, needed).await?),
        };
        Ok(Funding {
            sell_program: sell.program,
            decimals: sell.decimals,
            buy_program,
            source,
        })
    }

    /// The first `m` signers of the token program multisig at `authority`,
    /// enough to sign for it. Empty when `authority` is no multisig.
    async fn multisig_signers(&self, authority: Pubkey) -> Result<Vec<Pubkey>, Error> {
        let accounts = self.rpc.multiple_accounts([authority]).await?;
        let multisig = accounts
            .get(&authority)
            .filter(|account| TokenProgram::try_from(&account.owner).is_ok())
            .and_then(|account| Multisig::unpack(&account.data).ok());
        // An account that only looks like a multisig may claim more signers
        // than the array holds.
        Ok(multisig
            .and_then(|multisig| {
                multisig
                    .signers
                    .get(..usize::from(multisig.m))
                    .map(<[Pubkey]>::to_vec)
            })
            .unwrap_or_default())
    }

    /// Whether `mint`'s largest holders may be looked up again.
    fn holders_due(&self, mint: &Pubkey) -> bool {
        self.holders.is_some()
            && self
                .holder_accounts
                .get(mint)
                .is_none_or(|listed| listed.listed_at.elapsed() >= HOLDERS_REFRESH)
    }

    /// `mint`'s largest unfrozen, wallet-owned holders and their balances,
    /// largest first. They are looked up again when none holds `needed` and
    /// the lookup is due.
    async fn holders(&self, mint: Pubkey, needed: u64) -> Result<Vec<Holder>, Error> {
        if let Some(listed) = self.holder_accounts.get(&mint) {
            let holders = self.read_holders(&listed.accounts).await?;
            if holders.iter().any(|holder| holder.amount >= needed) || !self.holders_due(&mint) {
                return Ok(holders);
            }
        }
        let accounts = self.list_holders(mint).await;
        self.read_holders(&accounts).await
    }

    /// `mint`'s largest token accounts from the holders node. A lookup that
    /// fails, or has no node to ask, lists none until the next one is due.
    async fn list_holders(&self, mint: Pubkey) -> Vec<Pubkey> {
        let accounts = match &self.holders {
            Some(node) => node
                .token_largest_accounts(&mint)
                .await
                .inspect_err(|err| tracing::warn!(%mint, ?err, "largest holders not listed"))
                .unwrap_or_default(),
            None => {
                tracing::debug!(%mint, "no holders node, a mint nobody can mint stays unfunded");
                Vec::new()
            }
        };
        self.holder_accounts.insert(
            mint,
            HolderAccounts {
                accounts: accounts.clone(),
                listed_at: Instant::now(),
            },
        );
        accounts
    }

    /// The unfrozen, wallet-owned token accounts among `accounts`, in order,
    /// with their owners and balances.
    async fn read_holders(&self, accounts: &[Pubkey]) -> Result<Vec<Holder>, Error> {
        if accounts.is_empty() {
            return Ok(Vec::new());
        }
        let read = self.rpc.multiple_accounts(accounts.iter().copied()).await?;
        Ok(accounts
            .iter()
            .filter_map(|account| {
                let state =
                    StateWithExtensions::<TokenAccount>::unpack(&read.get(account)?.data).ok()?;
                // A PDA owner may be a pool on the route. Draining its vault
                // would move the pool's price.
                (state.base.state != AccountState::Frozen && state.base.owner.is_on_curve())
                    .then_some(Holder {
                        account: *account,
                        owner: state.base.owner,
                        amount: state.base.amount,
                    })
            })
            .collect())
    }
}

/// The accounts a transaction requires signatures from.
fn signers(transaction: &VersionedTransaction) -> &[Pubkey] {
    let keys = transaction.message.static_account_keys();
    let required = usize::from(transaction.message.header().num_required_signatures);
    &keys[..required.min(keys.len())]
}

fn mint_info(account: Option<&Account>) -> Option<MintInfo> {
    let account = account?;
    let program = TokenProgram::try_from(&account.owner).ok()?;
    let mint = StateWithExtensions::<Mint>::unpack(&account.data).ok()?;
    Some(MintInfo {
        program,
        decimals: mint.base.decimals,
        authority: mint.base.mint_authority.into(),
    })
}

/// An account's balance in `unit`, zero for a missing account or one that
/// is no token account.
fn balance(account: &Account, unit: Unit) -> u64 {
    match unit {
        Unit::Lamports => account.lamports,
        Unit::Tokens => StateWithExtensions::<TokenAccount>::unpack(&account.data)
            .map(|state| state.base.amount)
            .unwrap_or(0),
    }
}

/// The transaction standing in for the quoted order's creation: it makes the
/// sell token account, fills it with `fund`, delegates the sell amount to the
/// settlement state PDA and creates the order. `payer` pays the fee and the
/// rents, so the owner needs no lamports.
fn creation_leg(
    program_id: &Pubkey,
    payer: &Pubkey,
    order: &QuoteOrder,
    intent: &OrderIntent,
    fund: Instruction,
) -> VersionedTransaction {
    let token_program = order.sell_program.address();
    let instructions = [
        create_associated_token_account_idempotent(
            payer,
            &order.owner,
            &order.sell_mint,
            &token_program,
        ),
        fund,
        approve(
            &token_program,
            &order.sell_token_account(),
            &find_state_pda(program_id).0,
            &order.owner,
            &[],
            order.sell_amount,
        )
        .expect("a token program id passes the program check"),
        CreateOrder {
            program_id: *program_id,
            owner: order.owner,
            created_by: *payer,
            intent,
        }
        .into(),
    ];
    let message = VersionedMessage::Legacy(Message::new(&instructions, Some(payer)));
    VersionedTransaction {
        signatures: vec![Signature::default(); message.header().num_required_signatures.into()],
        message,
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Quote verifications by verdict and driver.
    #[metric(labels("verdict", "estimator_type"))]
    quote_verifications: prometheus::IntCounterVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
}

#[cfg(test)]
mod tests {
    use {super::*, solana_sdk::program_option::COption};

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn quoted() -> QuoteOrder {
        QuoteOrder {
            owner: pubkey(0x11),
            sell_mint: pubkey(0x33),
            sell_program: TokenProgram::SplToken,
            buy_mint: pubkey(0x44),
            buy_program: Some(TokenProgram::SplToken),
            sell_amount: 1_000,
            buy_amount: 2_000,
            kind: OrderKind::Sell,
        }
    }

    fn funding(source: Source) -> Funding {
        Funding {
            sell_program: TokenProgram::SplToken,
            decimals: 6,
            buy_program: Some(TokenProgram::SplToken),
            source,
        }
    }

    /// The program of each instruction, in order.
    fn programs(transaction: &VersionedTransaction) -> Vec<Pubkey> {
        let keys = transaction.message.static_account_keys();
        transaction
            .message
            .instructions()
            .iter()
            .map(|instruction| keys[usize::from(instruction.program_id_index)])
            .collect()
    }

    fn accounts(transaction: &VersionedTransaction, instruction: usize) -> Vec<Pubkey> {
        let keys = transaction.message.static_account_keys();
        transaction.message.instructions()[instruction]
            .accounts
            .iter()
            .map(|index| keys[usize::from(*index)])
            .collect()
    }

    /// The creation leg makes the owner's sell account, mints the sell amount
    /// into it, delegates that to the state PDA and creates the order. The
    /// payer, the owner and the authority's multisig signers sign, the
    /// authority itself does not.
    #[test]
    fn the_creation_leg_mints_delegates_and_creates() {
        let (program_id, payer, authority) = (pubkey(0xaa), pubkey(0xbb), pubkey(0xcc));
        let multisig = vec![pubkey(0xd1), pubkey(0xd2)];
        let order = quoted();
        let funding = funding(Source::Authority {
            authority,
            signers: multisig.clone(),
        });
        let fund = funding.instruction(&order).unwrap();
        let leg = creation_leg(&program_id, &payer, &order, &order.intent().unwrap(), fund);
        assert_eq!(
            programs(&leg),
            vec![
                spl_associated_token_account_interface::program::ID,
                spl_token_interface::ID,
                spl_token_interface::ID,
                program_id,
            ]
        );
        let mut signed = signers(&leg).to_vec();
        assert_eq!(signed.remove(0), payer);
        signed.sort();
        let mut expected = [vec![order.owner], multisig].concat();
        expected.sort();
        assert_eq!(signed, expected);
        assert_eq!(leg.signatures.len(), 4);
        assert_eq!(
            accounts(&leg, 1)[..2],
            [order.sell_mint, order.sell_token_account()]
        );
        assert_eq!(
            &leg.message.instructions()[1].data[1..],
            &1_000u64.to_le_bytes()
        );
        assert_eq!(
            accounts(&leg, 2),
            vec![
                order.sell_token_account(),
                find_state_pda(&program_id).0,
                order.owner,
            ]
        );
        assert_eq!(
            &leg.message.instructions()[2].data[1..],
            &1_000u64.to_le_bytes()
        );
        assert_eq!(accounts(&leg, 3)[..2], [order.owner, payer]);
    }

    /// The first holder holding the amount transfers it, and none holding it
    /// leaves the sell unfunded.
    #[test]
    fn the_first_holder_holding_the_amount_funds_the_sell() {
        let order = quoted();
        let holder = |byte: u8, amount| Holder {
            account: pubkey(byte),
            owner: pubkey(byte + 1),
            amount,
        };
        let (poor, rich) = (holder(0xe1, 999), holder(0xe3, 1_000));
        let fund = funding(Source::Holders(vec![poor.clone(), rich.clone()]))
            .instruction(&order)
            .unwrap();
        assert_eq!(fund.accounts[0].pubkey, rich.account);
        assert!(
            funding(Source::Holders(vec![poor]))
                .instruction(&order)
                .is_none()
        );
    }

    /// A mint reads as its program, decimals and authority, a revoked one as
    /// none, and anything else as no mint.
    #[test]
    fn a_mint_reads_as_its_authority() {
        let authority = pubkey(0x13);
        let mut data = vec![0; Mint::LEN];
        Mint {
            mint_authority: COption::Some(authority),
            is_initialized: true,
            decimals: 9,
            ..Mint::default()
        }
        .pack_into_slice(&mut data);
        let minted = Account {
            owner: spl_token_2022_interface::ID,
            data,
            ..Account::default()
        };
        let info = mint_info(Some(&minted)).unwrap();
        assert_eq!(info.program, TokenProgram::Token2022);
        assert_eq!(info.decimals, 9);
        assert_eq!(info.authority, Some(authority));
        let revoked = Account {
            owner: spl_token_interface::ID,
            ..solana_testlib::classic_mint(6)
        };
        assert_eq!(mint_info(Some(&revoked)).unwrap().authority, None);
        assert!(mint_info(None).is_none());
        assert!(mint_info(Some(&Account::default())).is_none());
    }
}
