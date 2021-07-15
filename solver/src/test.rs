///! Helper functions for unit tests.
use ethcontract::Account;

/// Create a dummy account.
pub fn account() -> Account {
    Account::Offline(
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
            .parse()
            .unwrap(),
        None,
    )
}
