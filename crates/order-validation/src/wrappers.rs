//! Wrappers listed in an order's app-data form a chain of calls: the first
//! wrapper is the target of the settlement transaction and the last one is
//! expected to call `settle()`. A wrapper that doesn't continue the chain makes
//! the transaction succeed without settling anything, so the order stays in
//! subsequent auctions and solvers pay gas for it over and over again.

use {
    alloy_contract::Error,
    alloy_primitives::Address,
    contracts::{GPv2AllowListAuthentication, ICowWrapper},
    ethrpc::alloy::errors::ContractErrorExt,
    std::collections::HashSet,
};

/// Checks that wrappers can be trusted to settle orders.
#[derive(Clone)]
pub struct Validator {
    authenticator: GPv2AllowListAuthentication::Instance,
    settlement: Address,
}

impl Validator {
    pub fn new(authenticator: GPv2AllowListAuthentication::Instance, settlement: Address) -> Self {
        Self {
            authenticator,
            settlement,
        }
    }

    /// Returns the first of the `wrappers` that can't be trusted to settle
    /// orders, if any. Errors if the RPC calls needed to decide that fail.
    ///
    /// Wrappers get checked one after another and the check stops at the first
    /// invalid one. That way an order listing lots of wrappers can only make us
    /// issue a few RPC calls since just the few legitimate wrappers let the
    /// check move on to the next one.
    pub async fn find_invalid(&self, wrappers: &[Address]) -> Result<Option<Address>, Error> {
        let mut checked = HashSet::new();
        for &wrapper in wrappers {
            if checked.insert(wrapper) && self.is_invalid(wrapper).await? {
                return Ok(Some(wrapper));
            }
        }
        Ok(None)
    }

    /// A wrapper is invalid if it isn't an authorized solver since only those
    /// may call `settle()`. Being authorized isn't sufficient though:
    /// solvers' submission accounts (EOAs, possibly with an EIP-7702
    /// delegation) are authorized as well and may accept arbitrary calls
    /// without settling anything. So a wrapper is also invalid if it doesn't
    /// identify as a `CowWrapper` of our settlement contract.
    async fn is_invalid(&self, wrapper: Address) -> Result<bool, Error> {
        if !self.authenticator.isSolver(wrapper).call().await? {
            return Ok(true);
        }
        let settlement = ICowWrapper::Instance::new(wrapper, self.authenticator.provider().clone())
            .SETTLEMENT()
            .call()
            .await;
        match settlement {
            Ok(settlement) => Ok(settlement != self.settlement),
            // Accounts that aren't wrappers revert, return no data (e.g. EOAs)
            // or return data that isn't an address. Any other error (e.g. a
            // node error) doesn't tell us anything about the wrapper.
            Err(err) if err.is_contract_revert() || matches!(err, Error::AbiError(_)) => Ok(true),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::Bytes,
        alloy_provider::mock::Asserter,
        alloy_sol_types::SolCall,
    };

    const SETTLEMENT: Address = Address::repeat_byte(0x90);
    const WRAPPER: Address = Address::repeat_byte(0x11);

    fn validator(asserter: Asserter) -> Validator {
        Validator::new(
            GPv2AllowListAuthentication::Instance::new(
                Address::repeat_byte(0xaa),
                ethrpc::Web3::with_asserter(asserter).provider,
            ),
            SETTLEMENT,
        )
    }

    fn is_solver(asserter: &Asserter, is_solver: bool) {
        asserter.push_success(&Bytes::from(
            GPv2AllowListAuthentication::GPv2AllowListAuthentication::isSolverCall::abi_encode_returns(
                &is_solver,
            ),
        ));
    }

    fn settlement(asserter: &Asserter, settlement: Address) {
        asserter.push_success(&Bytes::from(
            ICowWrapper::ICowWrapper::SETTLEMENTCall::abi_encode_returns(&settlement),
        ));
    }

    async fn find_invalid(asserter: Asserter) -> Result<Option<Address>, Error> {
        validator(asserter).find_invalid(&[WRAPPER]).await
    }

    #[tokio::test]
    async fn accepts_authorized_wrapper_of_settlement_contract() {
        let asserter = Asserter::new();
        is_solver(&asserter, true);
        settlement(&asserter, SETTLEMENT);
        assert_eq!(find_invalid(asserter).await.unwrap(), None);
    }

    #[tokio::test]
    async fn rejects_unauthorized_wrapper() {
        let asserter = Asserter::new();
        is_solver(&asserter, false);
        assert_eq!(find_invalid(asserter).await.unwrap(), Some(WRAPPER));
    }

    #[tokio::test]
    async fn rejects_wrapper_of_other_settlement_contract() {
        let asserter = Asserter::new();
        is_solver(&asserter, true);
        settlement(&asserter, Address::repeat_byte(0x91));
        assert_eq!(find_invalid(asserter).await.unwrap(), Some(WRAPPER));
    }

    #[tokio::test]
    async fn rejects_authorized_accounts_that_are_not_wrappers() {
        // EOA
        let asserter = Asserter::new();
        is_solver(&asserter, true);
        asserter.push_success(&Bytes::new());
        assert_eq!(find_invalid(asserter).await.unwrap(), Some(WRAPPER));

        // contract without `SETTLEMENT()`
        let asserter = Asserter::new();
        is_solver(&asserter, true);
        asserter.push_failure_msg("execution reverted");
        assert_eq!(find_invalid(asserter).await.unwrap(), Some(WRAPPER));

        // contract returning data that isn't an address
        let asserter = Asserter::new();
        is_solver(&asserter, true);
        asserter.push_success(&Bytes::from([0xff; 3]));
        assert_eq!(find_invalid(asserter).await.unwrap(), Some(WRAPPER));
    }

    #[tokio::test]
    async fn propagates_node_errors() {
        let asserter = Asserter::new();
        is_solver(&asserter, true);
        asserter.push_failure_msg("header not found");
        assert!(find_invalid(asserter).await.is_err());
    }

    #[tokio::test]
    async fn stops_at_first_invalid_wrapper() {
        // Responses for a single check; checking the second wrapper would fail
        // since the asserter has no responses left.
        let asserter = Asserter::new();
        is_solver(&asserter, false);
        let result = validator(asserter)
            .find_invalid(&[WRAPPER, Address::repeat_byte(0x12)])
            .await;
        assert_eq!(result.unwrap(), Some(WRAPPER));
    }

    #[tokio::test]
    async fn checks_each_wrapper_once() {
        // Responses for a single check; a second check would fail since the
        // asserter has no responses left.
        let asserter = Asserter::new();
        is_solver(&asserter, true);
        settlement(&asserter, SETTLEMENT);
        let result = validator(asserter).find_invalid(&[WRAPPER, WRAPPER]).await;
        assert_eq!(result.unwrap(), None);
    }
}
