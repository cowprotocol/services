//! Which mints the settlement program can move. It moves tokens with a plain
//! `Transfer`, which Token-2022 refuses for several mint extensions.

use {
    crate::infra::api::error,
    axum::http::StatusCode,
    solana_sdk::{account::Account, pubkey::Pubkey},
    spl_token_2022_interface::{
        extension::{
            BaseStateWithExtensions,
            StateWithExtensions,
            default_account_state::DefaultAccountState,
            non_transferable::NonTransferable,
            pausable::PausableConfig,
            transfer_fee::TransferFeeConfig,
            transfer_hook::TransferHook,
        },
        state::{AccountState, Mint},
    },
    std::{collections::HashMap, fmt},
};

/// Whether `program` is one of the two token programs, classic SPL or
/// Token-2022.
pub(super) fn is_token_program(program: &Pubkey) -> bool {
    *program == spl_token_interface::ID || *program == spl_token_2022_interface::ID
}

/// Answer `UnsupportedToken` for the first of `mints` the settlement program
/// cannot move, reading each mint from `accounts`.
pub(super) fn ensure_settleable(
    accounts: &HashMap<Pubkey, Account>,
    mints: impl IntoIterator<Item = Pubkey>,
) -> Result<(), error::Reply> {
    mints
        .into_iter()
        .try_for_each(|mint| match unsettleable_mint(accounts.get(&mint)) {
            Some(reason) => Err(error::reply(
                StatusCode::BAD_REQUEST,
                "UnsupportedToken",
                format!("Token {mint} is unsupported: {reason}"),
            )),
            None => Ok(()),
        })
}

/// Why the settlement program cannot move a mint's tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnsettleableMint {
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
/// TODO(BE-320): a permanent delegate mint passes, although its issuer can
/// move the buffer's balance of the token, retained fees included.
fn unsettleable_mint(account: Option<&Account>) -> Option<UnsettleableMint> {
    let Some(mint) = account
        .filter(|account| is_token_program(&account.owner))
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_sdk::program_pack::Pack,
        spl_token_2022_interface::extension::{
            BaseStateWithExtensionsMut,
            ExtensionType,
            StateWithExtensionsMut,
            mint_close_authority::MintCloseAuthority,
            permanent_delegate::PermanentDelegate,
        },
    };

    /// An initialized classic SPL Token mint.
    fn classic_mint() -> Account {
        let mut data = vec![0; Mint::LEN];
        Mint {
            is_initialized: true,
            decimals: 6,
            ..Mint::default()
        }
        .pack_into_slice(&mut data);
        Account {
            owner: spl_token_interface::ID,
            data,
            ..Account::default()
        }
    }

    /// An initialized Token-2022 mint with the `extensions`, whose values
    /// `init` sets.
    fn token_2022_mint(
        extensions: &[ExtensionType],
        init: impl FnOnce(&mut StateWithExtensionsMut<Mint>),
    ) -> Account {
        let len = ExtensionType::try_calculate_account_len::<Mint>(extensions).unwrap();
        let mut data = vec![0; len];
        let mut mint = StateWithExtensionsMut::<Mint>::unpack_uninitialized(&mut data).unwrap();
        init(&mut mint);
        mint.base = Mint {
            is_initialized: true,
            decimals: 6,
            ..Mint::default()
        };
        mint.pack_base();
        mint.init_account_type().unwrap();
        Account {
            owner: spl_token_2022_interface::ID,
            data,
            ..Account::default()
        }
    }

    /// The program moves classic mints and Token-2022 mints whose extensions
    /// leave a plain transfer alone, a permanent delegate included. Transfer
    /// fee, transfer hook and pausable mints fail it, paused or not, and so do
    /// non-transferable and frozen-by-default mints.
    #[test]
    fn classifies_mints_by_their_extensions() {
        let with = |extension, init: fn(&mut StateWithExtensionsMut<Mint>)| {
            unsettleable_mint(Some(&token_2022_mint(&[extension], init)))
        };
        assert_eq!(unsettleable_mint(Some(&classic_mint())), None);
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
        let mut token_account = vec![0; spl_token_interface::state::Account::LEN];
        spl_token_interface::state::Account {
            mint: Pubkey::new_unique(),
            owner: Pubkey::new_unique(),
            state: spl_token_interface::state::AccountState::Initialized,
            ..Default::default()
        }
        .pack_into_slice(&mut token_account);
        let token_account = Account {
            data: token_account,
            ..classic_mint()
        };
        let foreign = Account {
            owner: solana_system_interface::program::ID,
            ..classic_mint()
        };
        assert_eq!(unsettleable_mint(None), Some(UnsettleableMint::NotAMint));
        assert_eq!(
            unsettleable_mint(Some(&token_account)),
            Some(UnsettleableMint::NotAMint)
        );
        assert_eq!(
            unsettleable_mint(Some(&foreign)),
            Some(UnsettleableMint::NotAMint)
        );
    }

    /// The rejection names the mint and the reason, in the EVM
    /// `UnsupportedToken` shape.
    #[test]
    fn the_first_unsettleable_mint_is_rejected() {
        let good = Pubkey::new_unique();
        let bad = Pubkey::new_unique();
        let accounts = HashMap::from([
            (good, classic_mint()),
            (
                bad,
                token_2022_mint(&[ExtensionType::TransferFeeConfig], |mint| {
                    mint.init_extension::<TransferFeeConfig>(true).unwrap();
                }),
            ),
        ]);
        assert!(ensure_settleable(&accounts, [good]).is_ok());
        let (status, body) = ensure_settleable(&accounts, [good, bad]).unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.error_type, "UnsupportedToken");
        assert_eq!(
            body.description,
            format!("Token {bad} is unsupported: Token-2022 transfer fee extension")
        );
    }
}
