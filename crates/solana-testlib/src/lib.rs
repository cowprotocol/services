//! Shared test helpers for Solana CoW Protocol crates.

#![forbid(unsafe_code)]

use {
    base64::prelude::*,
    solana_sdk::{
        account::Account,
        program_pack::Pack,
        pubkey::Pubkey,
        signer::keypair::{Keypair, write_keypair_file},
    },
    spl_token_2022_interface::{
        extension::{BaseStateWithExtensionsMut, ExtensionType, StateWithExtensionsMut},
        state::{Account as TokenAccount, AccountState, Mint},
    },
    tempfile::NamedTempFile,
};

/// Write a fresh keypair to a temp file and return the handle.
///
/// The file is destroyed when the returned handle is dropped, so callers
/// must keep it alive for as long as it's needed.
pub fn temp_keypair() -> NamedTempFile {
    let file = NamedTempFile::new().expect("create temp file");
    write_keypair_file(&Keypair::new(), file.path()).expect("write keypair");
    file
}

/// An initialized mint of the classic SPL token program with `decimals`.
pub fn classic_mint(decimals: u8) -> Account {
    let mut data = vec![0; Mint::LEN];
    Mint {
        is_initialized: true,
        decimals,
        ..Mint::default()
    }
    .pack_into_slice(&mut data);
    Account {
        owner: spl_token_interface::ID,
        data,
        ..Account::default()
    }
}

/// An initialized Token-2022 mint with 6 decimals and the `extensions`, whose
/// values `init` sets.
pub fn token_2022_mint(
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

/// An initialized Token-2022 account of `mint` with the `extensions`, whose
/// values `init` sets.
pub fn token_2022_account(
    mint: &Pubkey,
    extensions: &[ExtensionType],
    init: impl FnOnce(&mut StateWithExtensionsMut<TokenAccount>),
) -> Account {
    let len = ExtensionType::try_calculate_account_len::<TokenAccount>(extensions).unwrap();
    let mut data = vec![0; len];
    let mut account =
        StateWithExtensionsMut::<TokenAccount>::unpack_uninitialized(&mut data).unwrap();
    init(&mut account);
    account.base = TokenAccount {
        mint: *mint,
        state: AccountState::Initialized,
        ..TokenAccount::default()
    };
    account.pack_base();
    account.init_account_type().unwrap();
    Account {
        owner: spl_token_2022_interface::ID,
        data,
        ..Account::default()
    }
}

/// `account` in the JSON shape a `getMultipleAccounts` mock answers with.
pub fn account_json(account: &Account) -> serde_json::Value {
    serde_json::json!({
        "lamports": account.lamports,
        "data": [BASE64_STANDARD.encode(&account.data), "base64"],
        "owner": account.owner.to_string(),
        "executable": account.executable,
        "rentEpoch": account.rent_epoch,
        "space": account.data.len(),
    })
}

/// An initialized SPL token account of `mint` owned by `owner`, in the JSON
/// shape a `getMultipleAccounts` mock answers with.
pub fn token_account_json(mint: &Pubkey, owner: &Pubkey) -> serde_json::Value {
    let mut data = vec![0; TokenAccount::LEN];
    TokenAccount {
        mint: *mint,
        owner: *owner,
        state: AccountState::Initialized,
        ..TokenAccount::default()
    }
    .pack_into_slice(&mut data);
    account_json(&Account {
        lamports: 2_039_280,
        owner: spl_token_interface::ID,
        data,
        ..Account::default()
    })
}

/// A `getMultipleAccounts` mock response, one entry per requested key in
/// request order: an account JSON or `null` for an account that does not
/// exist.
pub fn multiple_accounts_json(
    accounts: impl IntoIterator<Item = serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "context": {"slot": 1u64, "apiVersion": "2.0.0"},
        "value": accounts.into_iter().collect::<Vec<_>>(),
    })
}
