//! Shared test helpers for Solana CoW Protocol crates.

#![forbid(unsafe_code)]

use {
    base64::prelude::*,
    solana_sdk::{
        program_pack::Pack,
        pubkey::Pubkey,
        signer::keypair::{Keypair, write_keypair_file},
    },
    spl_token_interface::state::{Account as TokenAccount, AccountState},
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

/// An initialized SPL token account of `mint` owned by `owner`, in the JSON
/// shape a `getMultipleAccounts` mock answers with.
pub fn token_account_json(mint: &Pubkey, owner: &Pubkey) -> serde_json::Value {
    let mut data = [0u8; TokenAccount::LEN];
    TokenAccount {
        mint: *mint,
        owner: *owner,
        state: AccountState::Initialized,
        ..TokenAccount::default()
    }
    .pack_into_slice(&mut data);
    serde_json::json!({
        "lamports": 2_039_280u64,
        "data": [BASE64_STANDARD.encode(data), "base64"],
        "owner": spl_token_interface::ID.to_string(),
        "executable": false,
        "rentEpoch": 0u64,
        "space": TokenAccount::LEN,
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
