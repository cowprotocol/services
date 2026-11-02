//! Quote verification: each driver's quoted settlement is simulated after a
//! transaction that creates and funds the quoted order.

use {
    super::quoter::{Kind, Order, Quote},
    configs::price_estimation::QuoteVerificationMode,
    cow_settlement_client::instruction::CreateOrder,
    cow_settlement_interface::{
        data::{
            intent::{ENCODED_NATIVE_SOL_TRANSFER, OrderIntent, OrderKind},
            order::OrderAccount,
        },
        pda::{
            buffer::{find_buffer_pda, find_native_sol_buffer_pda},
            order::find_order_pda,
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
        instruction::{approve, mint_to},
        state::{Account as TokenAccount, Mint, Multisig},
    },
    std::time::Duration,
};

/// How long a pair's funding is reused: its mints and the sell mint's
/// authority. A failed creation leg drops it early.
const FUNDING_TTL: Duration = Duration::from_secs(2 * 60);

/// How many pairs the funding cache keeps.
const CACHE_CAPACITY: u64 = 10_000;

/// Settles quotes in simulation.
pub struct Verifier {
    /// Reads the mints and the accounts that fund them.
    rpc: SolanaRPC,
    /// Simulates the bundle, a node with the Jito `simulateBundle` extension.
    bundle: SolanaRPC,
    program_id: Pubkey,
    /// The share of the sell or buy amount the buffers may pay before the
    /// quote is dropped.
    inaccuracy_limit: f64,
    mode: QuoteVerificationMode,
    /// Each pair's funding, keyed by sell and buy mint.
    fundings: Cache<(Pubkey, Pubkey), Funding>,
}

/// What a quote's verification found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// The program records the order receiving the quoted amount, and the
    /// buffers pay less than the inaccuracy limit.
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
    /// The mint authority is revoked, so nobody mints them.
    Revoked,
}

impl Funding {
    /// The instruction filling `order`'s sell token account with its sell
    /// amount. `None` when nothing funds it.
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
            Source::Revoked => return None,
        };
        Some(instruction.expect("a token program id passes the program check"))
    }
}

/// A mint as the verification reads it.
struct MintInfo {
    program: TokenProgram,
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
    /// The buy amount an order account records as received.
    Received,
}

impl Verifier {
    pub fn new(
        rpc: SolanaRPC,
        bundle: SolanaRPC,
        program_id: Pubkey,
        inaccuracy_limit: f64,
        mode: QuoteVerificationMode,
    ) -> Self {
        Self {
            rpc,
            bundle,
            program_id,
            inaccuracy_limit,
            mode,
            fundings: Cache::builder()
                .time_to_live(FUNDING_TTL)
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
        let funding = self
            .funding(order)
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
    /// and read the fill the program records on the order and what the
    /// settlement did to the program's buffers.
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
            // Only a settlement through the program, which checks the solver
            // against its allowlist, records a fill on the order. A payment
            // that bypasses the program records none.
            (
                find_order_pda(&self.program_id, &intent.uid()).0,
                Unit::Received,
            ),
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

    /// What funds a simulated sell of the order's sell mint, the pair's cached
    /// read while it lasts.
    async fn funding(&self, order: &Order) -> Result<Funding, Error> {
        let key = (order.sell_token, order.buy_token);
        if let Some(funding) = self.fundings.get(&key) {
            return Ok(funding);
        }
        let funding = self.read_funding(order).await?;
        self.fundings.insert(key, funding.clone());
        Ok(funding)
    }

    /// What funds a simulated sell of the order's sell mint, read from the
    /// chain: its authority, unless revoked.
    async fn read_funding(&self, order: &Order) -> Result<Funding, Error> {
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
            None => Source::Revoked,
        };
        Ok(Funding {
            sell_program: sell.program,
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
        authority: mint.base.mint_authority.into(),
    })
}

/// An account's balance in `unit`, zero for a missing account or one that
/// is no token or order account.
fn balance(account: &Account, unit: Unit) -> u64 {
    match unit {
        Unit::Lamports => account.lamports,
        Unit::Tokens => StateWithExtensions::<TokenAccount>::unpack(&account.data)
            .map(|state| state.base.amount)
            .unwrap_or(0),
        Unit::Received => OrderAccount::attach(account.data.as_slice())
            .map(|order| order.filled_amounts().received)
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

    /// A mint reads as its program and authority, a revoked one as none, and
    /// anything else as no mint.
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
