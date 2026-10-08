//! Which mints and token accounts the settlement program can move tokens
//! through. Token-2022 tokens move with `TransferChecked`, so only an
//! extension's live setting blocks a mint: a charged fee, a hook program, a
//! pause, non-transferability, or new accounts starting frozen.

#![forbid(unsafe_code)]

use {
    cow_settlement_interface::token_program::TokenProgram,
    moka::sync::Cache,
    solana_sdk::{
        account::{Account, from_account},
        clock::Clock,
        program_pack::Pack,
        pubkey::Pubkey,
        rent::Rent,
        sysvar::clock,
    },
    spl_token_2022_interface::{
        extension::{
            BaseStateWithExtensions,
            ExtensionType,
            StateWithExtensions,
            confidential_transfer::ConfidentialTransferAccount,
            default_account_state::DefaultAccountState,
            memo_transfer::memo_required,
            non_transferable::NonTransferable,
            pausable::PausableConfig,
            transfer_fee::{TransferFee, TransferFeeConfig},
            transfer_hook::TransferHook,
        },
        state::{Account as TokenAccount, AccountState, Mint},
    },
    std::{collections::HashMap, fmt, time::Duration},
};

/// How long a mint's verdict stays cached. The pause switch, the fee schedule
/// and the default account state can change at any time, and a missing mint
/// can be created.
const VERDICT_TTL: Duration = Duration::from_secs(60);

/// How many mints the verdict cache holds.
const VERDICT_CAPACITY: u64 = 10_000;

/// Why the settlement program cannot move a mint's tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsettleableMint {
    /// The account is missing or not a mint of either token program.
    NotAMint,
    /// The mint charges a transfer fee, so the buy account would receive less
    /// than the pushed amount the program checks.
    TransferFee,
    /// A transfer hook program runs on every transfer, with extra accounts the
    /// settlement does not pass.
    TransferHook,
    /// The mint is paused, which rejects every transfer until it is unpaused.
    Paused,
    /// Token-2022 rejects every transfer of this mint's tokens.
    NonTransferable,
    /// New token accounts start frozen, so the buffer and payer account the
    /// settlement creates cannot receive the tokens.
    FrozenByDefault,
}

impl fmt::Display for UnsettleableMint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotAMint => "not a mint of the SPL Token or Token-2022 program",
            Self::TransferFee => "Token-2022 transfer fee",
            Self::TransferHook => "Token-2022 transfer hook program",
            Self::Paused => "Token-2022 mint is paused",
            Self::NonTransferable => "Token-2022 non-transferable extension",
            Self::FrozenByDefault => "new token accounts start frozen",
        })
    }
}

/// The settlement program's verdict on a mint: the token program it moves the
/// mint's tokens through, or why it cannot move them.
pub type MintVerdict = Result<TokenProgram, UnsettleableMint>;

/// The verdict on the mint at `account`, which is `None` when the account
/// does not exist. A mint is refused while its fee schedule live at `epoch`
/// charges or a charging newer schedule is pending, and without the epoch
/// while either schedule charges.
///
/// A permanent delegate mint passes, although its issuer can move the buffer's
/// balance of the token between settlements: PYUSD and every xStock carry one.
pub fn mint_verdict(account: Option<&Account>, epoch: Option<u64>) -> MintVerdict {
    let Some((program, mint)) = account.and_then(|account| {
        let program = TokenProgram::try_from(&account.owner).ok()?;
        let mint = StateWithExtensions::<Mint>::unpack(&account.data).ok()?;
        Some((program, mint))
    }) else {
        return Err(UnsettleableMint::NotAMint);
    };
    if mint
        .get_extension::<TransferFeeConfig>()
        .is_ok_and(|config| charges_fee(config, epoch))
    {
        Err(UnsettleableMint::TransferFee)
    } else if mint
        .get_extension::<TransferHook>()
        .is_ok_and(|hook| Option::<Pubkey>::from(hook.program_id).is_some())
    {
        Err(UnsettleableMint::TransferHook)
    } else if mint
        .get_extension::<PausableConfig>()
        .is_ok_and(|config| bool::from(config.paused))
    {
        Err(UnsettleableMint::Paused)
    } else if mint.get_extension::<NonTransferable>().is_ok() {
        Err(UnsettleableMint::NonTransferable)
    } else if mint
        .get_extension::<DefaultAccountState>()
        .is_ok_and(|default| default.state == AccountState::Frozen as u8)
    {
        Err(UnsettleableMint::FrozenByDefault)
    } else {
        Ok(program)
    }
}

/// Whether a transfer of the mint pays a fee at `epoch` or will once the
/// newer schedule starts. Token-2022 applies the newer schedule from its
/// epoch on and the older one before that, so without the epoch both count.
fn charges_fee(config: &TransferFeeConfig, epoch: Option<u64>) -> bool {
    let live = match epoch {
        Some(epoch) => config.get_epoch_fee(epoch),
        None => &config.older_transfer_fee,
    };
    takes_a_cut(live) || takes_a_cut(&config.newer_transfer_fee)
}

/// Whether a fee schedule takes anything from a transfer: a rate in basis
/// points under a positive cap.
fn takes_a_cut(fee: &TransferFee) -> bool {
    u16::from(fee.transfer_fee_basis_points) > 0 && u64::from(fee.maximum_fee) > 0
}

/// The verdicts on the mints read so far, each kept for [`VERDICT_TTL`].
/// Clones share one cache.
#[derive(Clone)]
pub struct MintVerdicts(Cache<Pubkey, MintVerdict>);

impl Default for MintVerdicts {
    fn default() -> Self {
        Self(
            Cache::builder()
                .time_to_live(VERDICT_TTL)
                .max_capacity(VERDICT_CAPACITY)
                .build(),
        )
    }
}

impl MintVerdicts {
    /// Start a lookup of `mints`: the cached verdicts are taken, the caller
    /// reads [`MintLookup::unread`] from the chain and hands the accounts to
    /// [`MintLookup::resolve`].
    pub fn lookup(&self, mints: impl IntoIterator<Item = Pubkey>) -> MintLookup<'_> {
        let mut lookup = MintLookup {
            cache: self,
            verdicts: HashMap::new(),
            unread: Vec::new(),
        };
        for mint in mints {
            match self.0.get(&mint) {
                Some(verdict) => {
                    lookup.verdicts.insert(mint, verdict);
                }
                None if lookup.unread.contains(&mint) => (),
                None => lookup.unread.push(mint),
            }
        }
        lookup
    }
}

/// A lookup of mints in progress, see [`MintVerdicts::lookup`].
pub struct MintLookup<'a> {
    cache: &'a MintVerdicts,
    verdicts: HashMap<Pubkey, MintVerdict>,
    unread: Vec<Pubkey>,
}

impl MintLookup<'_> {
    /// The accounts for the caller's chain read: the mints without a cached
    /// verdict, in first-seen order, then the Clock sysvar whose epoch picks
    /// each mint's live fee schedule. Nothing when every verdict was cached.
    pub fn unread(&self) -> impl Iterator<Item = Pubkey> + '_ {
        let clock = (!self.unread.is_empty()).then_some(clock::ID);
        self.unread.iter().copied().chain(clock)
    }

    /// Every mint's verdict. The unread mints are judged from `accounts`, a
    /// mint absent from it as missing, and their verdicts are cached. The
    /// epoch comes from the Clock sysvar in `accounts`, if it is there.
    pub fn resolve(mut self, accounts: &HashMap<Pubkey, Account>) -> HashMap<Pubkey, MintVerdict> {
        let epoch = accounts
            .get(&clock::ID)
            .and_then(from_account::<Clock, _>)
            .map(|clock| clock.epoch);
        for mint in self.unread {
            let verdict = mint_verdict(accounts.get(&mint), epoch);
            self.cache.0.insert(mint, verdict);
            self.verdicts.insert(mint, verdict);
        }
        self.verdicts
    }
}

/// The rent-exempt minimum under `rent`, in lamports, of a new associated
/// token account of the mint at `mint`, `None` when it is no mint of either
/// token program. A Token-2022 one carries the immutable owner extension and
/// the account extensions the mint's own extensions require.
pub fn ata_rent(rent: &Rent, mint: &Account) -> Option<u64> {
    let len = match TokenProgram::try_from(&mint.owner).ok()? {
        // A classic mint is exactly 82 bytes; any other account of the
        // program is a token account or a multisig.
        TokenProgram::SplToken if mint.data.len() == Mint::LEN => TokenAccount::LEN,
        TokenProgram::SplToken => return None,
        TokenProgram::Token2022 => {
            let mint = StateWithExtensions::<Mint>::unpack(&mint.data).ok()?;
            token_2022_ata_len(&mint.get_extension_types().ok()?)
        }
    };
    Some(rent.minimum_balance(len))
}

/// The [`ata_rent`] of the largest associated token account a mint passing
/// [`mint_verdict`] can need: under a Token-2022 mint whose fee schedule takes
/// nothing, whose hook names no program and whose pause switch is off.
pub fn max_ata_rent(rent: &Rent) -> u64 {
    rent.minimum_balance(token_2022_ata_len(&[
        ExtensionType::TransferFeeConfig,
        ExtensionType::TransferHook,
        ExtensionType::Pausable,
    ]))
}

fn token_2022_ata_len(mint_extensions: &[ExtensionType]) -> usize {
    let mut extensions = ExtensionType::get_required_init_account_extensions(mint_extensions);
    extensions.push(ExtensionType::ImmutableOwner);
    ExtensionType::try_calculate_account_len::<TokenAccount>(&extensions)
        .expect("account extensions have fixed lengths")
}

/// Whether the settlement's plain `Transfer` of `mint` lands in the token
/// account at `account`, see [`receivable_token_account_owner`].
pub fn receivable_token_account(account: &Account, mint: &Pubkey) -> bool {
    receivable_token_account_owner(account, mint).is_some()
}

/// The owner of the token account at `account` when the settlement's plain
/// `Transfer` of `mint` lands in it: initialized, unfrozen, holding `mint`,
/// and not set to refuse transfers without a memo or outside confidential
/// balances. `None` when it does not.
pub fn receivable_token_account_owner(account: &Account, mint: &Pubkey) -> Option<Pubkey> {
    TokenProgram::try_from(&account.owner).ok()?;
    let state = StateWithExtensions::<TokenAccount>::unpack(&account.data).ok()?;
    (state.base.state == AccountState::Initialized
        && state.base.mint == *mint
        && !memo_required(&state)
        && !refuses_non_confidential_credits(&state))
    .then_some(state.base.owner)
}

/// Whether the account's confidential transfer extension refuses credits to
/// its regular balance.
fn refuses_non_confidential_credits(state: &StateWithExtensions<'_, TokenAccount>) -> bool {
    state
        .get_extension::<ConfidentialTransferAccount>()
        .is_ok_and(|confidential| !bool::from(&confidential.allow_non_confidential_credits))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_sdk::{account::create_account_for_test, clock::Clock},
        solana_testlib::{classic_mint, token_2022_account, token_2022_mint},
        spl_token_2022_interface::extension::{
            BaseStateWithExtensionsMut,
            StateWithExtensionsMut,
            confidential_transfer::ConfidentialTransferMint,
            memo_transfer::MemoTransfer,
            mint_close_authority::MintCloseAuthority,
            permanent_delegate::PermanentDelegate,
        },
    };

    /// A fee schedule from `epoch` on that takes nothing.
    fn no_fee(epoch: u64) -> TransferFee {
        TransferFee {
            epoch: epoch.into(),
            ..TransferFee::default()
        }
    }

    /// A fee schedule from `epoch` on that takes 1% of a transfer, at most
    /// 1,000 atoms.
    fn one_percent(epoch: u64) -> TransferFee {
        TransferFee {
            epoch: epoch.into(),
            transfer_fee_basis_points: 100.into(),
            maximum_fee: 1_000.into(),
        }
    }

    /// A Token-2022 mint charging `older` until `newer` takes over at its
    /// epoch.
    fn fee_mint(older: TransferFee, newer: TransferFee) -> Account {
        token_2022_mint(&[ExtensionType::TransferFeeConfig], |mint| {
            let config = mint.init_extension::<TransferFeeConfig>(true).unwrap();
            config.older_transfer_fee = older;
            config.newer_transfer_fee = newer;
        })
    }

    /// The Clock sysvar account at `epoch`.
    fn clock_at(epoch: u64) -> Account {
        create_account_for_test(&Clock {
            epoch,
            ..Clock::default()
        })
    }

    /// The program moves classic mints and Token-2022 mints whose extension
    /// settings leave the transfer whole, each under its own token program: a
    /// permanent delegate, a fee schedule taking nothing, a hook without a
    /// program and a pausable mint while not paused. A charged fee, a hook
    /// program, a pause, a non-transferable mint and frozen-by-default
    /// accounts fail it.
    #[test]
    fn classifies_mints_by_their_extensions() {
        let judge = |account: &Account| mint_verdict(Some(account), Some(100));
        let with = |extension, init: fn(&mut StateWithExtensionsMut<Mint>)| {
            judge(&token_2022_mint(&[extension], init))
        };
        assert_eq!(judge(&classic_mint(6)), Ok(TokenProgram::SplToken));
        assert_eq!(
            judge(&token_2022_mint(&[], |_| {})),
            Ok(TokenProgram::Token2022)
        );
        assert_eq!(
            with(ExtensionType::MintCloseAuthority, |mint| {
                mint.init_extension::<MintCloseAuthority>(true).unwrap();
            }),
            Ok(TokenProgram::Token2022)
        );
        assert_eq!(
            with(ExtensionType::PermanentDelegate, |mint| {
                mint.init_extension::<PermanentDelegate>(true)
                    .unwrap()
                    .delegate = Some(Pubkey::new_unique()).try_into().unwrap();
            }),
            Ok(TokenProgram::Token2022)
        );
        assert_eq!(
            judge(&fee_mint(no_fee(0), no_fee(0))),
            Ok(TokenProgram::Token2022)
        );
        assert_eq!(
            judge(&fee_mint(
                no_fee(0),
                TransferFee {
                    maximum_fee: 0.into(),
                    ..one_percent(0)
                }
            )),
            Ok(TokenProgram::Token2022)
        );
        assert_eq!(
            judge(&fee_mint(no_fee(0), one_percent(0))),
            Err(UnsettleableMint::TransferFee)
        );
        assert_eq!(
            with(ExtensionType::TransferHook, |mint| {
                mint.init_extension::<TransferHook>(true).unwrap();
            }),
            Ok(TokenProgram::Token2022)
        );
        assert_eq!(
            with(ExtensionType::TransferHook, |mint| {
                mint.init_extension::<TransferHook>(true)
                    .unwrap()
                    .program_id = Some(Pubkey::new_unique()).try_into().unwrap();
            }),
            Err(UnsettleableMint::TransferHook)
        );
        assert_eq!(
            with(ExtensionType::Pausable, |mint| {
                mint.init_extension::<PausableConfig>(true).unwrap();
            }),
            Ok(TokenProgram::Token2022)
        );
        assert_eq!(
            with(ExtensionType::Pausable, |mint| {
                mint.init_extension::<PausableConfig>(true).unwrap().paused = true.into();
            }),
            Err(UnsettleableMint::Paused)
        );
        assert_eq!(
            with(ExtensionType::NonTransferable, |mint| {
                mint.init_extension::<NonTransferable>(true).unwrap();
            }),
            Err(UnsettleableMint::NonTransferable)
        );
        assert_eq!(
            with(ExtensionType::DefaultAccountState, |mint| {
                mint.init_extension::<DefaultAccountState>(true)
                    .unwrap()
                    .state = AccountState::Frozen as u8;
            }),
            Err(UnsettleableMint::FrozenByDefault)
        );
        assert_eq!(
            with(ExtensionType::DefaultAccountState, |mint| {
                mint.init_extension::<DefaultAccountState>(true)
                    .unwrap()
                    .state = AccountState::Initialized as u8;
            }),
            Ok(TokenProgram::Token2022)
        );
    }

    /// A mint is refused while the schedule live at the epoch charges or a
    /// charging newer schedule is pending: a dropped fee stops counting once
    /// its zero schedule is live, and a scheduled fee counts before its
    /// epoch. Without the epoch both schedules count.
    #[test]
    fn a_fee_counts_while_live_or_pending() {
        let dropped = fee_mint(one_percent(0), no_fee(10));
        let scheduled = fee_mint(no_fee(0), one_percent(10));
        assert_eq!(
            mint_verdict(Some(&dropped), Some(10)),
            Ok(TokenProgram::Token2022)
        );
        assert_eq!(
            mint_verdict(Some(&dropped), Some(9)),
            Err(UnsettleableMint::TransferFee)
        );
        assert_eq!(
            mint_verdict(Some(&dropped), None),
            Err(UnsettleableMint::TransferFee)
        );
        assert_eq!(
            mint_verdict(Some(&scheduled), Some(9)),
            Err(UnsettleableMint::TransferFee)
        );
        assert_eq!(
            mint_verdict(Some(&scheduled), Some(10)),
            Err(UnsettleableMint::TransferFee)
        );
        assert_eq!(
            mint_verdict(Some(&scheduled), None),
            Err(UnsettleableMint::TransferFee)
        );
    }

    /// A missing account, a token account and a mint layout under another
    /// program are no mints the program can move.
    #[test]
    fn only_token_program_mints_are_settleable() {
        let foreign = Account {
            owner: Pubkey::new_unique(),
            ..classic_mint(6)
        };
        assert_eq!(
            mint_verdict(None, Some(100)),
            Err(UnsettleableMint::NotAMint)
        );
        assert_eq!(
            mint_verdict(
                Some(&token_2022_account(&Pubkey::new_unique(), &[], |_| {})),
                Some(100)
            ),
            Err(UnsettleableMint::NotAMint)
        );
        assert_eq!(
            mint_verdict(Some(&foreign), Some(100)),
            Err(UnsettleableMint::NotAMint)
        );
    }

    /// A lookup reads the mints without a cached verdict and the Clock
    /// sysvar after them, whose epoch judges a dropped fee as gone. The
    /// second lookup reads only the mint the first one did not judge: the
    /// cached verdicts stand without an account, and a mint missing from the
    /// accounts handed in is judged missing. A lookup with every verdict
    /// cached reads nothing.
    #[test]
    fn a_lookup_reads_the_clock_with_the_mints_without_a_cached_verdict() {
        let cache = MintVerdicts::default();
        let (classic, dropped, absent) = (
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        let lookup = cache.lookup([classic, dropped, classic]);
        assert_eq!(
            lookup.unread().collect::<Vec<_>>(),
            [classic, dropped, clock::ID]
        );
        let verdicts = lookup.resolve(&HashMap::from([
            (classic, classic_mint(6)),
            (dropped, fee_mint(one_percent(0), no_fee(10))),
            (clock::ID, clock_at(10)),
        ]));
        assert_eq!(
            verdicts,
            HashMap::from([
                (classic, Ok(TokenProgram::SplToken)),
                (dropped, Ok(TokenProgram::Token2022)),
            ])
        );

        let lookup = cache.lookup([classic, dropped, absent]);
        assert_eq!(lookup.unread().collect::<Vec<_>>(), [absent, clock::ID]);
        assert_eq!(
            lookup.resolve(&HashMap::new()),
            HashMap::from([
                (classic, Ok(TokenProgram::SplToken)),
                (dropped, Ok(TokenProgram::Token2022)),
                (absent, Err(UnsettleableMint::NotAMint)),
            ])
        );

        let lookup = cache.lookup([classic, dropped]);
        assert_eq!(lookup.unread().count(), 0);
    }

    /// Without a readable clock a lookup refuses a mint while either of its
    /// schedules charges.
    #[test]
    fn a_lookup_without_the_clock_refuses_either_charging_schedule() {
        let dropped = Pubkey::new_unique();
        let cache = MintVerdicts::default();
        let lookup = cache.lookup([dropped]);
        assert_eq!(
            lookup.resolve(&HashMap::from([(
                dropped,
                fee_mint(one_percent(0), no_fee(10))
            )])),
            HashMap::from([(dropped, Err(UnsettleableMint::TransferFee))])
        );
    }

    /// A rent of one lamport per byte, so a rent-exempt minimum reads as the
    /// account's length plus the 128-byte storage overhead.
    fn rent_per_byte() -> Rent {
        Rent::with_lamports_per_byte(1)
    }

    /// A classic associated token account takes 165 bytes, a Token-2022 one
    /// 170 with its immutable owner extension, plus the account extensions
    /// the mint's own extensions add even when they leave the transfer whole.
    #[test]
    fn ata_rent_grows_with_the_account_extensions_the_mint_adds() {
        let rent = &rent_per_byte();
        assert_eq!(ata_rent(rent, &classic_mint(6)), Some(128 + 165));
        assert_eq!(
            ata_rent(&Rent::default(), &classic_mint(6)),
            Some(2_039_280)
        );
        assert_eq!(
            ata_rent(rent, &token_2022_mint(&[], |_| {})),
            Some(128 + 170)
        );
        assert_eq!(
            ata_rent(rent, &fee_mint(no_fee(0), no_fee(0))),
            Some(128 + 182)
        );
        let largest = token_2022_mint(
            &[
                ExtensionType::TransferFeeConfig,
                ExtensionType::TransferHook,
                ExtensionType::Pausable,
            ],
            |mint| {
                mint.init_extension::<TransferFeeConfig>(true).unwrap();
                mint.init_extension::<TransferHook>(true).unwrap();
                mint.init_extension::<PausableConfig>(true).unwrap();
            },
        );
        assert_eq!(
            mint_verdict(Some(&largest), None),
            Ok(TokenProgram::Token2022)
        );
        assert_eq!(ata_rent(rent, &largest), Some(max_ata_rent(rent)));
        assert_eq!(max_ata_rent(rent), 128 + 191);
        assert_eq!(
            ata_rent(
                rent,
                &token_2022_account(&Pubkey::new_unique(), &[], |_| {})
            ),
            None
        );
        let classic_account = Account {
            owner: TokenProgram::SplToken.address(),
            data: vec![0; TokenAccount::LEN],
            ..Account::default()
        };
        assert_eq!(ata_rent(rent, &classic_account), None);
    }

    /// A confidential transfer mint asks nothing of its accounts at creation —
    /// the confidential extension is configured on each account afterwards —
    /// so the rent stays the plain Token-2022 one.
    #[test]
    fn confidential_mints_keep_the_plain_token_2022_rent() {
        let mint = token_2022_mint(&[ExtensionType::ConfidentialTransferMint], |mint| {
            mint.init_extension::<ConfidentialTransferMint>(true)
                .unwrap();
        });
        assert_eq!(mint_verdict(Some(&mint), None), Ok(TokenProgram::Token2022));
        let rent = &rent_per_byte();
        assert_eq!(ata_rent(rent, &mint), Some(128 + 170));
        assert!(ata_rent(rent, &mint) <= Some(max_ata_rent(rent)));
    }

    /// A Token-2022 account receives the payout unless it holds another mint,
    /// requires memos on incoming transfers, or refuses credits outside its
    /// confidential balance.
    #[test]
    fn token_2022_accounts_receive_unless_they_refuse_plain_transfers() {
        let mint = Pubkey::new_unique();
        let with_memos = |required: bool| {
            token_2022_account(&mint, &[ExtensionType::MemoTransfer], |account| {
                account
                    .init_extension::<MemoTransfer>(true)
                    .unwrap()
                    .require_incoming_transfer_memos = required.into();
            })
        };
        let with_confidential = |allow_plain: bool| {
            token_2022_account(
                &mint,
                &[ExtensionType::ConfidentialTransferAccount],
                |account| {
                    account
                        .init_extension::<ConfidentialTransferAccount>(true)
                        .unwrap()
                        .allow_non_confidential_credits = allow_plain.into();
                },
            )
        };
        assert!(receivable_token_account(
            &token_2022_account(&mint, &[], |_| {}),
            &mint
        ));
        assert!(!receivable_token_account(
            &token_2022_account(&Pubkey::new_unique(), &[], |_| {}),
            &mint
        ));
        assert!(receivable_token_account(&with_memos(false), &mint));
        assert!(!receivable_token_account(&with_memos(true), &mint));
        assert!(receivable_token_account(&with_confidential(true), &mint));
        assert!(!receivable_token_account(&with_confidential(false), &mint));
    }
}
