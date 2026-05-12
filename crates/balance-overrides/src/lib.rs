pub mod balance;
pub mod detector;

pub use balance::BalanceOverrideRequest;
use {alloy_primitives::Address, alloy_rpc_types::state::AccountOverride};

/// A component that can provide ERC-20 balance state overrides.
///
/// This allows a wider range of verified quotes to work, even when balances
/// are not available for the quoter.
#[async_trait::async_trait]
pub trait StateOverriding: Send + Sync + 'static {
    async fn balance_override(
        &self,
        request: BalanceOverrideRequest,
    ) -> Option<(Address, AccountOverride)>;
}

/// The default state override provider, handling ERC-20 balance overrides.
#[derive(Debug)]
pub struct StateOverrides {
    pub(crate) balance_detector: balance::Detector,
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
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            balance::{Strategy, aave::ray_div},
            detector::mapping_slot_hash,
        },
        alloy_primitives::{U256, address, b256},
        ethrpc::{Web3, mock},
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
                alloy_rpc_types::state::AccountOverride {
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
                alloy_rpc_types::state::AccountOverride {
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
        use cached::Cached;

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
        use cached::Cached;

        let token = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let holder1 = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let holder2 = address!("0000000000000000000000000000000000000001");
        let target_contract = address!("0000000000000000000000000000000000000002");

        let strategy_h1 = Strategy::DirectSlot {
            target_contract,
            slot: alloy_primitives::B256::repeat_byte(1),
        };
        let strategy_h2 = Strategy::DirectSlot {
            target_contract,
            slot: alloy_primitives::B256::repeat_byte(2),
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
    async fn aave_v3_a_token_override_scales_amount_and_writes_low_128() {
        use alloy_provider::mock::Asserter;

        let a_token = address!("4d5F47FA6A74757f35C14fD3a6Ef8E3C9BC514E8");
        let pool = address!("87870bca3f3fd6335c3f4ce8392d69350b4fa4e2");
        let underlying = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let holder = address!("18709E89BD403F470088aBDAcEbE86CC60dda12e");
        let amount = U256::from(1_000_000_000_000_000_000u128);

        let asserter = Asserter::new();
        let index = U256::from_str_radix("1063000000000000000000000000", 10).unwrap();
        asserter.push_success(&format!("0x{:064x}", index));

        let web3 = Web3::with_asserter(asserter);
        let strategy = Strategy::AaveV3AToken {
            target_contract: a_token,
            pool,
            underlying,
            web3,
        };

        let (addr, override_) = strategy
            .state_override(&holder, &amount)
            .await
            .into_iter()
            .last()
            .expect("override computed");

        assert_eq!(addr, a_token);

        let diff = override_.state_diff.expect("state diff present");
        assert_eq!(diff.len(), 1);
        let (slot, value) = diff.into_iter().next().unwrap();
        assert_eq!(
            slot,
            b256!("6785743a4ad9de6e692f819936c9d0b94b199ed36f2660e82404737b769718e5")
        );
        let word = U256::from_be_bytes(value.0);
        assert_eq!(word >> 128, U256::ZERO);
        assert_eq!(word, ray_div(amount, index).unwrap());
    }

    #[test]
    fn ray_div_edge_cases() {
        let index = U256::from_str_radix("1063000000000000000000000000", 10).unwrap();
        assert_eq!(ray_div(U256::ZERO, index).unwrap(), U256::ZERO);
        assert_eq!(
            ray_div(U256::from(1_000_000_000_000_000_000u128), U256::ZERO),
            None,
        );
    }

    #[test]
    fn mapping_slot_hash_matches_solidity_layout() {
        let holder = address!("18709E89BD403F470088aBDAcEbE86CC60dda12e");
        let slot = mapping_slot_hash(&holder, &U256::from(52).to_be_bytes::<32>());
        assert_eq!(
            slot,
            b256!("6785743a4ad9de6e692f819936c9d0b94b199ed36f2660e82404737b769718e5")
        );
    }
}
