//! Which mints and token accounts the settlement program can move tokens
//! through. The program moves tokens with a plain `Transfer`, which Token-2022
//! refuses for several mint and account extensions.

#![forbid(unsafe_code)]

use {
    cow_settlement_interface::token_program::TokenProgram,
    moka::sync::Cache,
    solana_sdk::{account::Account, pubkey::Pubkey},
    spl_token_2022_interface::{
        extension::{
            BaseStateWithExtensions,
            StateWithExtensions,
            confidential_transfer::ConfidentialTransferAccount,
            default_account_state::DefaultAccountState,
            memo_transfer::memo_required,
            non_transferable::NonTransferable,
            pausable::PausableConfig,
            transfer_fee::TransferFeeConfig,
            transfer_hook::TransferHook,
        },
        state::{Account as TokenAccount, AccountState, Mint},
    },
    std::{collections::HashMap, fmt, time::Duration},
};

/// How long a mint's verdict stays cached. The freeze authority can flip the
/// default account state at any time, and a missing mint can be created.
const VERDICT_TTL: Duration = Duration::from_secs(60);

/// How many mints the verdict cache holds.
const VERDICT_CAPACITY: u64 = 10_000;

/// Why the settlement program cannot move a mint's tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsettleableMint {
    /// The account is missing or not a mint of either token program.
    NotAMint,
    /// Token-2022 rejects the program's plain `Transfer` for this mint, even
    /// at a zero fee.
    TransferFee,
    /// Token-2022 rejects the program's plain `Transfer` for this mint, even
    /// without a hook program.
    TransferHook,
    /// Token-2022 rejects the program's plain `Transfer` for this mint, even
    /// while it is not paused.
    Pausable,
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
            Self::TransferFee => "Token-2022 transfer fee extension",
            Self::TransferHook => "Token-2022 transfer hook extension",
            Self::Pausable => "Token-2022 pausable extension",
            Self::NonTransferable => "Token-2022 non-transferable extension",
            Self::FrozenByDefault => "new token accounts start frozen",
        })
    }
}

/// The settlement program's verdict on a mint: the token program it moves the
/// mint's tokens through, or why it cannot move them.
pub type MintVerdict = Result<TokenProgram, UnsettleableMint>;

/// The verdict on the mint at `account`, which is `None` when the account
/// does not exist.
///
/// TODO(BE-344): a permanent delegate mint passes, although its issuer can
/// move the buffer's balance of the token, retained fees included.
pub fn mint_verdict(account: Option<&Account>) -> MintVerdict {
    let Some((program, mint)) = account.and_then(|account| {
        let program = TokenProgram::try_from(&account.owner).ok()?;
        let mint = StateWithExtensions::<Mint>::unpack(&account.data).ok()?;
        Some((program, mint))
    }) else {
        return Err(UnsettleableMint::NotAMint);
    };
    if mint.get_extension::<TransferFeeConfig>().is_ok() {
        Err(UnsettleableMint::TransferFee)
    } else if mint.get_extension::<TransferHook>().is_ok() {
        Err(UnsettleableMint::TransferHook)
    } else if mint.get_extension::<PausableConfig>().is_ok() {
        Err(UnsettleableMint::Pausable)
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
    /// The mints without a cached verdict, in first-seen order, for the
    /// caller's chain read.
    pub fn unread(&self) -> impl Iterator<Item = Pubkey> + '_ {
        self.unread.iter().copied()
    }

    /// Every mint's verdict. The unread mints are judged from `accounts`, a
    /// mint absent from it as missing, and their verdicts are cached.
    pub fn resolve(mut self, accounts: &HashMap<Pubkey, Account>) -> HashMap<Pubkey, MintVerdict> {
        for mint in self.unread {
            let verdict = mint_verdict(accounts.get(&mint));
            self.cache.0.insert(mint, verdict);
            self.verdicts.insert(mint, verdict);
        }
        self.verdicts
    }
}

/// The rent-exempt minimum, in lamports, of an associated token account
/// under `program` at the SDK's default rent: 165 bytes under SPL Token, 170
/// under Token-2022, whose associated token accounts carry the immutable owner
/// extension.
pub fn ata_rent(program: TokenProgram) -> u64 {
    match program {
        TokenProgram::SplToken => 2_039_280,
        TokenProgram::Token2022 => 2_074_080,
    }
}

/// Whether the settlement's plain `Transfer` of `mint` lands in the token
/// account at `account`: initialized, unfrozen, holding `mint`, and not set
/// to refuse transfers without a memo or outside confidential balances.
pub fn receivable_token_account(account: &Account, mint: &Pubkey) -> bool {
    TokenProgram::try_from(&account.owner).is_ok()
        && StateWithExtensions::<TokenAccount>::unpack(&account.data).is_ok_and(|state| {
            state.base.state == AccountState::Initialized
                && state.base.mint == *mint
                && !memo_required(&state)
                && !refuses_non_confidential_credits(&state)
        })
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
        solana_sdk::{program_pack::Pack, rent::Rent},
        solana_testlib::{classic_mint, token_2022_account, token_2022_mint},
        spl_token_2022_interface::extension::{
            BaseStateWithExtensionsMut,
            ExtensionType,
            StateWithExtensionsMut,
            memo_transfer::MemoTransfer,
            mint_close_authority::MintCloseAuthority,
            permanent_delegate::PermanentDelegate,
        },
    };

    /// The program moves classic mints and Token-2022 mints whose extensions
    /// leave a plain transfer alone, a permanent delegate included, each under
    /// its own token program. Transfer fee, transfer hook and pausable mints
    /// fail it, paused or not, and so do non-transferable and
    /// frozen-by-default mints.
    #[test]
    fn classifies_mints_by_their_extensions() {
        let with = |extension, init: fn(&mut StateWithExtensionsMut<Mint>)| {
            mint_verdict(Some(&token_2022_mint(&[extension], init)))
        };
        assert_eq!(
            mint_verdict(Some(&classic_mint(6))),
            Ok(TokenProgram::SplToken)
        );
        assert_eq!(
            mint_verdict(Some(&token_2022_mint(&[], |_| {}))),
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
            with(ExtensionType::TransferFeeConfig, |mint| {
                mint.init_extension::<TransferFeeConfig>(true).unwrap();
            }),
            Err(UnsettleableMint::TransferFee)
        );
        assert_eq!(
            with(ExtensionType::TransferHook, |mint| {
                mint.init_extension::<TransferHook>(true).unwrap();
            }),
            Err(UnsettleableMint::TransferHook)
        );
        assert_eq!(
            with(ExtensionType::Pausable, |mint| {
                mint.init_extension::<PausableConfig>(true).unwrap().paused = true.into();
            }),
            Err(UnsettleableMint::Pausable)
        );
        assert_eq!(
            with(ExtensionType::Pausable, |mint| {
                mint.init_extension::<PausableConfig>(true).unwrap();
            }),
            Err(UnsettleableMint::Pausable)
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

    /// A missing account, a token account and a mint layout under another
    /// program are no mints the program can move.
    #[test]
    fn only_token_program_mints_are_settleable() {
        let foreign = Account {
            owner: Pubkey::new_unique(),
            ..classic_mint(6)
        };
        assert_eq!(mint_verdict(None), Err(UnsettleableMint::NotAMint));
        assert_eq!(
            mint_verdict(Some(&token_2022_account(
                &Pubkey::new_unique(),
                &[],
                |_| {}
            ))),
            Err(UnsettleableMint::NotAMint)
        );
        assert_eq!(
            mint_verdict(Some(&foreign)),
            Err(UnsettleableMint::NotAMint)
        );
    }

    /// The second lookup reads only the mint the first one did not judge:
    /// the cached verdicts stand without an account, and a mint missing from
    /// the accounts handed in is judged missing.
    #[test]
    fn a_lookup_reads_only_the_mints_without_a_cached_verdict() {
        let cache = MintVerdicts::default();
        let (classic, fee, absent) = (
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        let fee_mint = token_2022_mint(&[ExtensionType::TransferFeeConfig], |mint| {
            mint.init_extension::<TransferFeeConfig>(true).unwrap();
        });

        let lookup = cache.lookup([classic, fee, classic]);
        assert_eq!(lookup.unread().collect::<Vec<_>>(), [classic, fee]);
        let verdicts = lookup.resolve(&HashMap::from([
            (classic, classic_mint(6)),
            (fee, fee_mint),
        ]));
        assert_eq!(
            verdicts,
            HashMap::from([
                (classic, Ok(TokenProgram::SplToken)),
                (fee, Err(UnsettleableMint::TransferFee)),
            ])
        );

        let lookup = cache.lookup([classic, fee, absent]);
        assert_eq!(lookup.unread().collect::<Vec<_>>(), [absent]);
        assert_eq!(
            lookup.resolve(&HashMap::new()),
            HashMap::from([
                (classic, Ok(TokenProgram::SplToken)),
                (fee, Err(UnsettleableMint::TransferFee)),
                (absent, Err(UnsettleableMint::NotAMint)),
            ])
        );
    }

    #[test]
    fn ata_rents_are_the_default_minimums_of_the_account_layouts() {
        let rent = Rent::default();
        assert_eq!(
            ata_rent(TokenProgram::SplToken),
            rent.minimum_balance(TokenAccount::LEN)
        );
        let token_2022_len = ExtensionType::try_calculate_account_len::<TokenAccount>(&[
            ExtensionType::ImmutableOwner,
        ])
        .unwrap();
        assert_eq!(
            ata_rent(TokenProgram::Token2022),
            rent.minimum_balance(token_2022_len)
        );
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
