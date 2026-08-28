//! Typed views of fetched on-chain accounts.
//!
//! Raw account state (owners, data bytes) stays behind the blockchain
//! adapter. The domain receives decoded lookup tables and classified
//! token-account states, never raw `Account`s.

use {
    crate::infra::blockchain::token::{SPL_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID},
    solana_address_lookup_table_interface::state::AddressLookupTable,
    solana_sdk::{account::Account, message::AddressLookupTableAccount, pubkey::Pubkey},
    std::collections::HashMap,
};

/// A point-in-time snapshot of accounts from one batched fetch.
///
/// An account that does not exist on chain is not in the snapshot. Each
/// interpretation method (ALT, Token account) reports a missing account for its
/// key.
pub struct AccountsSnapshot {
    accounts: HashMap<Pubkey, Account>,
}

impl AccountsSnapshot {
    pub(super) fn new(accounts: HashMap<Pubkey, Account>) -> Self {
        Self { accounts }
    }

    /// Return the address lookup table at `key` for the v0 message compiler.
    ///
    /// # Requirements
    ///
    /// - The account must exist.
    /// - The address-lookup-table program must own the account.
    /// - The table must be active. Reject tables in the deactivation cool-down.
    ///   The cool-down can finish before the transaction lands, and then the
    ///   compiled indexes become stale.
    ///
    /// # Why these checks matter
    ///
    /// `MessageV0::try_compile` runs in this driver and does not read chain
    /// state. It only sees the addresses that this method returns. The
    /// Solana runtime resolves the real table account when it executes the
    /// transaction. A missing, wrongly owned, inactive, or deactivating
    /// table passes compilation and then fails on-chain after submission.
    /// These checks reject bad tables before the driver sends the
    /// transaction.
    pub fn lookup_table(
        &self,
        key: &Pubkey,
    ) -> Result<AddressLookupTableAccount, InvalidAddressLookupTableReason> {
        use InvalidAddressLookupTableReason::*;
        let account = self.accounts.get(key).ok_or(AccountNotFound)?;
        if account.owner != solana_address_lookup_table_interface::program::id() {
            return Err(UnexpectedOwner);
        }
        let table =
            AddressLookupTable::deserialize(&account.data).map_err(|_| DeserializeFailed)?;
        if table.meta.deactivation_slot != u64::MAX {
            return Err(Deactivated);
        }
        Ok(AddressLookupTableAccount {
            key: *key,
            addresses: table.addresses.to_vec(),
        })
    }

    /// Classify the state of the token account at `address` for a caller that
    /// creates missing token accounts idempotently.
    pub fn token_account_state(&self, address: &Pubkey) -> TokenAccountState {
        match self.accounts.get(address) {
            None => TokenAccountState::NeedsCreation,
            Some(account) if account.owner == SPL_TOKEN_PROGRAM_ID => {
                TokenAccountState::Initialized
            }
            Some(account) if account.owner == SYSTEM_PROGRAM_ID && account.data.is_empty() => {
                TokenAccountState::NeedsCreation
            }
            Some(account) => TokenAccountState::Unexpected {
                owner: account.owner,
                data_len: account.data.len(),
            },
        }
    }
}

/// The observed state of a token account, for a caller that creates missing
/// token accounts idempotently.
#[derive(Debug, Clone, Copy)]
pub enum TokenAccountState {
    /// The caller must create the account before it can hold tokens. The
    /// account does not exist, or it is a pre-funded system-owned account with
    /// no data.
    NeedsCreation,
    /// An initialized SPL token account. The caller does not need to create
    /// it.
    Initialized,
    /// Any other state: a foreign owner, or a system account with data. The
    /// caller cannot use this account, and an idempotent create cannot
    /// replace it.
    Unexpected { owner: Pubkey, data_len: usize },
}

/// Why the snapshot rejected an account as an address lookup table.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum InvalidAddressLookupTableReason {
    /// The account does not exist on chain.
    #[error("account not found")]
    AccountNotFound,
    /// The address-lookup-table program does not own the account.
    #[error("unexpected owner")]
    UnexpectedOwner,
    /// The account data does not deserialize as an address lookup table.
    #[error("failed to deserialize")]
    DeserializeFailed,
    /// The table is deactivated or deactivating.
    #[error("deactivated")]
    Deactivated,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_address_lookup_table_interface::state::LookupTableMeta,
        std::borrow::Cow,
    };

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn snapshot(entries: impl IntoIterator<Item = (Pubkey, Account)>) -> AccountsSnapshot {
        AccountsSnapshot::new(entries.into_iter().collect())
    }

    /// A serialized lookup table that contains `addresses` and has the given
    /// `deactivation_slot`. A slot of `u64::MAX` means the table is active.
    fn serialized_table(deactivation_slot: u64, addresses: &[Pubkey]) -> Vec<u8> {
        AddressLookupTable {
            meta: LookupTableMeta {
                deactivation_slot,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Borrowed(addresses),
        }
        .serialize_for_tests()
        .unwrap()
    }

    fn table_account(data: Vec<u8>) -> Account {
        Account {
            owner: solana_address_lookup_table_interface::program::id(),
            data,
            ..Account::default()
        }
    }

    #[test]
    fn resolves_an_active_lookup_table() {
        let key = pubkey(0x11);
        let addresses = vec![pubkey(0x22), pubkey(0x33)];
        let snapshot = snapshot([(key, table_account(serialized_table(u64::MAX, &addresses)))]);

        let table = snapshot.lookup_table(&key).unwrap();
        assert_eq!(table.key, key);
        assert_eq!(table.addresses, addresses);
    }

    #[test]
    fn rejects_a_missing_lookup_table() {
        let err = snapshot([]).lookup_table(&pubkey(0x11)).unwrap_err();
        assert!(matches!(
            err,
            InvalidAddressLookupTableReason::AccountNotFound
        ));
    }

    #[test]
    fn rejects_a_lookup_table_with_a_foreign_owner() {
        let key = pubkey(0x11);
        let account = Account {
            owner: pubkey(0xff),
            data: serialized_table(u64::MAX, &[]),
            ..Account::default()
        };
        let err = snapshot([(key, account)]).lookup_table(&key).unwrap_err();
        assert!(matches!(
            err,
            InvalidAddressLookupTableReason::UnexpectedOwner
        ));
    }

    #[test]
    fn rejects_a_lookup_table_that_fails_to_deserialize() {
        let key = pubkey(0x11);
        let account = table_account(vec![0xff; 4]);
        let err = snapshot([(key, account)]).lookup_table(&key).unwrap_err();
        assert!(matches!(
            err,
            InvalidAddressLookupTableReason::DeserializeFailed
        ));
    }

    #[test]
    fn rejects_a_deactivated_lookup_table() {
        let key = pubkey(0x11);
        let account = table_account(serialized_table(5, &[pubkey(0x22)]));
        let err = snapshot([(key, account)]).lookup_table(&key).unwrap_err();
        assert!(matches!(err, InvalidAddressLookupTableReason::Deactivated));
    }

    #[test]
    fn a_missing_token_account_needs_creation() {
        let state = snapshot([]).token_account_state(&pubkey(0x11));
        assert!(matches!(state, TokenAccountState::NeedsCreation));
    }

    #[test]
    fn a_token_owned_account_is_initialized() {
        let address = pubkey(0x11);
        let account = Account {
            owner: SPL_TOKEN_PROGRAM_ID,
            ..Account::default()
        };
        let state = snapshot([(address, account)]).token_account_state(&address);
        assert!(matches!(state, TokenAccountState::Initialized));
    }

    #[test]
    fn a_prefunded_system_account_needs_creation() {
        let address = pubkey(0x11);
        let account = Account {
            owner: SYSTEM_PROGRAM_ID,
            lamports: 1_000,
            ..Account::default()
        };
        let state = snapshot([(address, account)]).token_account_state(&address);
        assert!(matches!(state, TokenAccountState::NeedsCreation));
    }

    #[test]
    fn a_foreign_owned_account_is_unexpected() {
        let address = pubkey(0x11);
        let owner = pubkey(0xff);
        let account = Account {
            owner,
            data: vec![1, 2, 3],
            ..Account::default()
        };
        let state = snapshot([(address, account)]).token_account_state(&address);
        assert!(
            matches!(state, TokenAccountState::Unexpected { owner: o, data_len: 3 } if o == owner)
        );
    }

    #[test]
    fn a_system_account_with_data_is_unexpected() {
        let address = pubkey(0x11);
        let account = Account {
            owner: SYSTEM_PROGRAM_ID,
            data: vec![0; 8],
            ..Account::default()
        };
        let state = snapshot([(address, account)]).token_account_state(&address);
        assert!(matches!(
            state,
            TokenAccountState::Unexpected { data_len: 8, .. }
        ));
    }
}
