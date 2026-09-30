//! Which mints and token accounts the settlement program can move tokens
//! through. The program moves tokens with a plain `Transfer`, which Token-2022
//! refuses for several mint and account extensions.

#![forbid(unsafe_code)]

use {
    cow_settlement_interface::token_program::TokenProgram,
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
    std::fmt,
};

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

/// Why the settlement program cannot move the tokens of the mint at
/// `account`, `None` when it can.
///
/// TODO(BE-344): a permanent delegate mint passes, although its issuer can
/// move the buffer's balance of the token, retained fees included.
pub fn unsettleable_mint(account: Option<&Account>) -> Option<UnsettleableMint> {
    let Some(mint) = account
        .filter(|account| TokenProgram::try_from(&account.owner).is_ok())
        .and_then(|account| StateWithExtensions::<Mint>::unpack(&account.data).ok())
    else {
        return Some(UnsettleableMint::NotAMint);
    };
    if mint.get_extension::<TransferFeeConfig>().is_ok() {
        Some(UnsettleableMint::TransferFee)
    } else if mint.get_extension::<TransferHook>().is_ok() {
        Some(UnsettleableMint::TransferHook)
    } else if mint.get_extension::<PausableConfig>().is_ok() {
        Some(UnsettleableMint::Pausable)
    } else if mint.get_extension::<NonTransferable>().is_ok() {
        Some(UnsettleableMint::NonTransferable)
    } else if mint
        .get_extension::<DefaultAccountState>()
        .is_ok_and(|default| default.state == AccountState::Frozen as u8)
    {
        Some(UnsettleableMint::FrozenByDefault)
    } else {
        None
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
    /// leave a plain transfer alone, a permanent delegate included. Transfer
    /// fee, transfer hook and pausable mints fail it, paused or not, and so do
    /// non-transferable and frozen-by-default mints.
    #[test]
    fn classifies_mints_by_their_extensions() {
        let with = |extension, init: fn(&mut StateWithExtensionsMut<Mint>)| {
            unsettleable_mint(Some(&token_2022_mint(&[extension], init)))
        };
        assert_eq!(unsettleable_mint(Some(&classic_mint(6))), None);
        assert_eq!(unsettleable_mint(Some(&token_2022_mint(&[], |_| {}))), None);
        assert_eq!(
            with(ExtensionType::MintCloseAuthority, |mint| {
                mint.init_extension::<MintCloseAuthority>(true).unwrap();
            }),
            None
        );
        assert_eq!(
            with(ExtensionType::PermanentDelegate, |mint| {
                mint.init_extension::<PermanentDelegate>(true)
                    .unwrap()
                    .delegate = Some(Pubkey::new_unique()).try_into().unwrap();
            }),
            None
        );
        assert_eq!(
            with(ExtensionType::TransferFeeConfig, |mint| {
                mint.init_extension::<TransferFeeConfig>(true).unwrap();
            }),
            Some(UnsettleableMint::TransferFee)
        );
        assert_eq!(
            with(ExtensionType::TransferHook, |mint| {
                mint.init_extension::<TransferHook>(true).unwrap();
            }),
            Some(UnsettleableMint::TransferHook)
        );
        assert_eq!(
            with(ExtensionType::Pausable, |mint| {
                mint.init_extension::<PausableConfig>(true).unwrap().paused = true.into();
            }),
            Some(UnsettleableMint::Pausable)
        );
        assert_eq!(
            with(ExtensionType::Pausable, |mint| {
                mint.init_extension::<PausableConfig>(true).unwrap();
            }),
            Some(UnsettleableMint::Pausable)
        );
        assert_eq!(
            with(ExtensionType::NonTransferable, |mint| {
                mint.init_extension::<NonTransferable>(true).unwrap();
            }),
            Some(UnsettleableMint::NonTransferable)
        );
        assert_eq!(
            with(ExtensionType::DefaultAccountState, |mint| {
                mint.init_extension::<DefaultAccountState>(true)
                    .unwrap()
                    .state = AccountState::Frozen as u8;
            }),
            Some(UnsettleableMint::FrozenByDefault)
        );
        assert_eq!(
            with(ExtensionType::DefaultAccountState, |mint| {
                mint.init_extension::<DefaultAccountState>(true)
                    .unwrap()
                    .state = AccountState::Initialized as u8;
            }),
            None
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
        assert_eq!(unsettleable_mint(None), Some(UnsettleableMint::NotAMint));
        assert_eq!(
            unsettleable_mint(Some(&token_2022_account(
                &Pubkey::new_unique(),
                &[],
                |_| {}
            ))),
            Some(UnsettleableMint::NotAMint)
        );
        assert_eq!(
            unsettleable_mint(Some(&foreign)),
            Some(UnsettleableMint::NotAMint)
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
