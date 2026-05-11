pub mod approval;
pub mod balance;
pub mod detector;

use {alloy_primitives::Address, alloy_rpc_types::state::AccountOverride};
pub use {
    approval::{ApprovalOverrideRequest, ApprovalStrategy},
    balance::BalanceOverrideRequest,
};

/// A component that can provide ERC-20 balance and allowance state overrides.
///
/// This allows a wider range of verified quotes to work, even when balances or
/// approvals are not available for the quoter.
#[async_trait::async_trait]
pub trait StateOverriding: Send + Sync + 'static {
    async fn balance_override(
        &self,
        request: BalanceOverrideRequest,
    ) -> Option<(Address, AccountOverride)>;

    async fn approval_override(
        &self,
        request: ApprovalOverrideRequest,
    ) -> Option<(Address, AccountOverride)>;
}

/// The default state override provider, handling both ERC-20 balance and
/// allowance overrides.
#[derive(Debug)]
pub struct StateOverrides {
    pub(crate) balance_detector: balance::Detector,
    pub(crate) approval_detector: approval::Detector,
}

impl StateOverrides {
    /// Creates a new instance with default detection parameters.
    pub fn new(web3: ethrpc::Web3) -> Self {
        Self::with_config(web3, 60, detector::DEFAULT_VERIFICATION_TIMEOUT, 1000)
    }

    /// Creates a new instance with custom detection parameters.
    pub fn with_config(
        web3: ethrpc::Web3,
        probing_depth: u8,
        verification_timeout: std::time::Duration,
        cache_size: usize,
    ) -> Self {
        Self {
            balance_detector: balance::Detector::new(
                web3.clone(),
                probing_depth,
                verification_timeout,
                cache_size,
            ),
            approval_detector: approval::Detector::new(
                web3,
                probing_depth,
                verification_timeout,
                cache_size,
            ),
        }
    }
}

#[async_trait::async_trait]
impl StateOverriding for StateOverrides {
    async fn balance_override(
        &self,
        request: BalanceOverrideRequest,
    ) -> Option<(Address, AccountOverride)> {
        let strategy = self
            .balance_detector
            .detect(request.token, request.holder)
            .await?;

        strategy
            .state_override(&request.holder, &request.amount)
            .await
            .into_iter()
            .last()
    }

    async fn approval_override(
        &self,
        request: ApprovalOverrideRequest,
    ) -> Option<(Address, AccountOverride)> {
        let strategy = self
            .approval_detector
            .detect(request.token, request.owner, request.spender)
            .await?;
        strategy
            .state_override(request.owner, request.spender, request.amount)
            .into_iter()
            .last()
    }
}

/// State overrider that always returns `None`. Useful for testing.
pub struct DummyStateOverrider;

#[async_trait::async_trait]
impl StateOverriding for DummyStateOverrider {
    async fn balance_override(
        &self,
        _request: BalanceOverrideRequest,
    ) -> Option<(Address, AccountOverride)> {
        None
    }

    async fn approval_override(
        &self,
        _request: ApprovalOverrideRequest,
    ) -> Option<(Address, AccountOverride)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::balance::Strategy,
        alloy_primitives::{B256, U256, address, b256},
        cached::Cached,
        ethrpc::mock,
    };

    #[tokio::test]
    async fn balance_override_computation() {
        let cow = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let holder = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let amount = U256::from(0x42);
        let strategy = Strategy::SolidityMapping {
            target_contract: cow,
            map_slot: U256::from(0),
        };

        let result = strategy
            .state_override(&holder, &amount)
            .await
            .into_iter()
            .last();
        assert_eq!(
            result,
            Some((
                cow,
                AccountOverride {
                    state_diff: Some(
                        std::iter::once((
                            b256!(
                                "fca351f4d96129454cfc8ef7930b638ac71fea35eb69ee3b8d959496beb04a33"
                            ),
                            b256!(
                                "0000000000000000000000000000000000000000000000000000000000000042"
                            )
                        ))
                        .collect()
                    ),
                    ..Default::default()
                }
            )),
        );
    }

    #[tokio::test]
    async fn balance_overrides_none_for_unknown_tokens() {
        let state_overrides = DummyStateOverrider;
        assert_eq!(
            state_overrides
                .balance_override(BalanceOverrideRequest {
                    token: address!("0000000000000000000000000000000000000000"),
                    holder: address!("0000000000000000000000000000000000000001"),
                    amount: U256::ZERO,
                })
                .await,
            None,
        );
    }

    #[tokio::test]
    async fn balance_override_computation_solady() {
        let token = address!("0000000000c5dc95539589fbd24be07c6c14eca4");
        let holder = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let amount = U256::from(0x42);
        let strategy = Strategy::SoladyMapping {
            target_contract: address!("0000000000c5dc95539589fbd24be07c6c14eca4"),
        };

        let result = strategy
            .state_override(&holder, &amount)
            .await
            .into_iter()
            .last();
        assert_eq!(
            result,
            Some((
                token,
                AccountOverride {
                    state_diff: Some({
                        std::iter::once((
                            b256!(
                                "f6a6656ed2d14bad3cdd3e8871db3f535a136a1b6cd5ae2dced8eb813f3d4e4f"
                            ),
                            b256!(
                                "0000000000000000000000000000000000000000000000000000000000000042"
                            ),
                        ))
                        .collect()
                    }),
                    ..Default::default()
                }
            )),
        );
    }

    #[tokio::test]
    async fn cached_detection_caches_holder_agnostic_strategies_without_holder() {
        let token = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let holder1 = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let holder2 = address!("0000000000000000000000000000000000000001");
        let target_contract = address!("0000000000000000000000000000000000000002");

        let strategy = Strategy::SolidityMapping {
            target_contract,
            map_slot: U256::from(3),
        };

        let mock_web3 = mock::web3();
        let state_overrides = StateOverrides::new(mock_web3);

        {
            state_overrides
                .balance_detector
                .cache
                .lock()
                .unwrap()
                .cache_set((token, None), Some(strategy.clone()));
        }

        assert_eq!(
            state_overrides
                .balance_detector
                .detect(token, holder1)
                .await,
            Some(strategy.clone())
        );
        assert_eq!(
            state_overrides
                .balance_detector
                .detect(token, holder2)
                .await,
            Some(strategy)
        );
    }

    #[tokio::test]
    async fn cached_detection_caches_holder_specific_strategies_with_holder() {
        let token = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let holder1 = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let holder2 = address!("0000000000000000000000000000000000000001");
        let target_contract = address!("0000000000000000000000000000000000000002");

        let strategy_h1 = Strategy::DirectSlot {
            target_contract,
            slot: B256::repeat_byte(1),
        };
        let strategy_h2 = Strategy::DirectSlot {
            target_contract,
            slot: B256::repeat_byte(2),
        };

        let mock_web3 = mock::web3();
        let state_overrides = StateOverrides::new(mock_web3);

        {
            state_overrides
                .balance_detector
                .cache
                .lock()
                .unwrap()
                .cache_set((token, Some(holder1)), Some(strategy_h1.clone()));
            state_overrides
                .balance_detector
                .cache
                .lock()
                .unwrap()
                .cache_set((token, Some(holder2)), Some(strategy_h2.clone()));
        }

        assert_eq!(
            state_overrides
                .balance_detector
                .detect(token, holder1)
                .await,
            Some(strategy_h1)
        );
        assert_eq!(
            state_overrides
                .balance_detector
                .detect(token, holder2)
                .await,
            Some(strategy_h2)
        );
    }

    #[tokio::test]
    async fn cached_approval_detection_caches_pair_agnostic_strategies_without_pair() {
        use cached::Cached;

        let token = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let owner1 = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let spender1 = address!("0000000000000000000000000000000000000001");
        let owner2 = address!("0000000000000000000000000000000000000002");
        let spender2 = address!("0000000000000000000000000000000000000003");
        let target_contract = address!("0000000000000000000000000000000000000004");

        let strategy = ApprovalStrategy::SolidityMappingOwnerToSpender {
            target_contract,
            map_slot: U256::from(1),
        };

        let mock_web3 = mock::web3();
        let state_overrides = StateOverrides::new(mock_web3);

        {
            state_overrides
                .approval_detector
                .cache
                .lock()
                .unwrap()
                .cache_set((token, None), Some(strategy.clone()));
        }

        assert_eq!(
            state_overrides
                .approval_detector
                .detect(token, owner1, spender1)
                .await,
            Some(strategy.clone())
        );
        assert_eq!(
            state_overrides
                .approval_detector
                .detect(token, owner2, spender2)
                .await,
            Some(strategy)
        );
    }

    #[tokio::test]
    async fn cached_approval_detection_caches_pair_specific_strategies_with_pair() {
        let token = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let owner1 = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let spender1 = address!("0000000000000000000000000000000000000001");
        let owner2 = address!("0000000000000000000000000000000000000002");
        let spender2 = address!("0000000000000000000000000000000000000003");
        let target_contract = address!("0000000000000000000000000000000000000004");

        let strategy_p1 = ApprovalStrategy::DirectSlot {
            target_contract,
            slot: alloy_primitives::B256::repeat_byte(1),
        };
        let strategy_p2 = ApprovalStrategy::DirectSlot {
            target_contract,
            slot: alloy_primitives::B256::repeat_byte(2),
        };

        let mock_web3 = mock::web3();
        let state_overrides = StateOverrides::new(mock_web3);

        {
            state_overrides
                .approval_detector
                .cache
                .lock()
                .unwrap()
                .cache_set((token, Some((owner1, spender1))), Some(strategy_p1.clone()));
            state_overrides
                .approval_detector
                .cache
                .lock()
                .unwrap()
                .cache_set((token, Some((owner2, spender2))), Some(strategy_p2.clone()));
        }

        assert_eq!(
            state_overrides
                .approval_detector
                .detect(token, owner1, spender1)
                .await,
            Some(strategy_p1)
        );
        assert_eq!(
            state_overrides
                .approval_detector
                .detect(token, owner2, spender2)
                .await,
            Some(strategy_p2)
        );
    }
}
