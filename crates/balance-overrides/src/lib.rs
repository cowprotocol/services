mod aave;
pub mod detector;

use {
    self::{
        aave::mapping_slot_hash,
        detector::{DetectionError, Detector},
    },
    alloy_primitives::{Address, B256, U256, keccak256, map::AddressMap},
    alloy_rpc_types::state::AccountOverride,
    cached::{Cached, SizedCache},
    configs::balance_overrides::Strategy,
    ethrpc::Web3,
    std::{collections::HashMap, iter, sync::Mutex},
};
/// Token configurations for the `BalanceOverriding` component.
#[derive(Clone, Debug, Default)]
pub struct TokenConfiguration(HashMap<Address, Strategy>);

impl TokenConfiguration {
    pub fn new(configuration: HashMap<Address, Strategy>) -> Self {
        Self(configuration)
    }

    pub fn into_inner(self) -> HashMap<Address, Strategy> {
        self.0
    }
}

/// A component that can provide balance overrides for tokens.
///
/// This allows a wider range of verified quotes to work, even when balances
/// are not available for the quoter.
#[async_trait::async_trait]
pub trait BalanceOverriding: Send + Sync + 'static {
    async fn state_override(
        &self,
        request: BalanceOverrideRequest,
    ) -> Option<(Address, AccountOverride)>;
}

/// Parameters for computing a balance override request.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct BalanceOverrideRequest {
    /// The token for the override.
    pub token: Address,
    /// The account to override the balance for.
    pub holder: Address,
    /// The token amount (in atoms) to set the balance to.
    pub amount: U256,
}

#[async_trait::async_trait]
trait StrategyExt {
    /// Computes the storage slot and value to override for a particular token
    /// holder and amount. `web3` is only consulted by strategies that need
    /// on-chain reads at override time (currently `AaveV3AToken`); other
    /// variants ignore it and complete synchronously.
    async fn state_override(
        &self,
        web3: Option<&Web3>,
        holder: &Address,
        amount: &U256,
    ) -> AddressMap<AccountOverride>;

    fn is_valid_for_all_holders(&self) -> bool;
}

#[async_trait::async_trait]
impl StrategyExt for Strategy {
    async fn state_override(
        &self,
        web3: Option<&Web3>,
        holder: &Address,
        amount: &U256,
    ) -> AddressMap<AccountOverride> {
        let (target_contract, key) = match self {
            Self::SolidityMapping {
                target_contract,
                map_slot,
            } => (
                *target_contract,
                mapping_slot_hash(holder, &map_slot.to_be_bytes()),
            ),
            Self::SoladyMapping { target_contract } => {
                let mut buf = [0; 32];
                buf[0..20].copy_from_slice(holder.as_slice());
                buf[28..32].copy_from_slice(&[0x87, 0xa2, 0x11, 0xa2]);
                (*target_contract, keccak256(buf))
            }
            Self::DirectSlot {
                target_contract,
                slot,
            } => (*target_contract, *slot),
            Self::AaveV3AToken {
                target_contract,
                pool,
                underlying,
            } => {
                let Some(web3) = web3 else {
                    tracing::warn!(
                        ?target_contract,
                        "AaveV3AToken balance override requested but web3 is not configured",
                    );
                    return AddressMap::default();
                };
                return match aave::build_override(
                    web3,
                    *target_contract,
                    *pool,
                    *underlying,
                    *holder,
                    *amount,
                )
                .await
                {
                    Some((addr, override_)) => iter::once((addr, override_)).collect(),
                    None => AddressMap::default(),
                };
            }
        };

        let state_override = AccountOverride {
            state_diff: Some(iter::once((key, B256::new(amount.to_be_bytes::<32>()))).collect()),
            ..Default::default()
        };

        iter::once((target_contract, state_override)).collect()
    }

    fn is_valid_for_all_holders(&self) -> bool {
        // `AaveV3AToken` fields (target, pool, underlying) are all token-level
        // constants; the slot and value are derived per-holder at override
        // time. Caching the strategy once per token avoids re-running the
        // probe for every new `from` address.
        matches!(self, Self::DirectSlot { .. } | Self::AaveV3AToken { .. })
    }
}

type DetectorCache = Mutex<SizedCache<(Address, Option<Address>), Option<Strategy>>>;

/// The default balance override provider.
#[derive(Debug, Default)]
pub struct BalanceOverrides {
    /// The configured balance override strategies for tokens.
    ///
    /// These take priority over the auto-detection mechanism and are excluded
    /// from the cache in order to prevent them from getting cleaned up by
    /// the caching policy.
    pub hardcoded: HashMap<Address, Strategy>,
    /// The balance override detector and its cache. Set to `None` if
    /// auto-detection is disabled. The detector internally holds its own
    /// `Web3` handle for tracing and verification calls.
    pub detector: Option<(Detector, DetectorCache)>,
    /// `Web3` handle used by strategies that need on-chain reads at
    /// override-resolution time (currently only `AaveV3AToken`, which
    /// fetches the live liquidity index from the Aave pool). Kept separate
    /// from the detector's own web3 so that hardcoded `AaveV3AToken`
    /// entries still resolve when auto-detection is disabled. Both handles
    /// typically point at the same underlying `Web3` instance; `Web3` is
    /// cheaply cloneable, so the double-reference is just two `Arc` bumps.
    pub web3: Option<Web3>,
}

impl BalanceOverrides {
    /// Creates a new instance with sensible defaults.
    pub fn new(web3: ethrpc::Web3) -> Self {
        Self {
            hardcoded: Default::default(),
            detector: Some((
                Detector::new(web3.clone(), 60, detector::DEFAULT_VERIFICATION_TIMEOUT),
                Mutex::new(SizedCache::with_size(1000)),
            )),
            web3: Some(web3),
        }
    }

    pub(crate) async fn cached_detection(
        &self,
        token: Address,
        holder: Address,
    ) -> Option<Strategy> {
        let (detector, cache) = self.detector.as_ref()?;
        tracing::trace!(?token, "attempting to auto-detect");

        {
            let mut cache = cache.lock().unwrap();
            if let Some(strategy) = cache.cache_get(&(token, None)) {
                tracing::trace!(?token, "cache hit (strategy valid for all holders)");
                return strategy.clone();
            }
            if let Some(strategy) = cache.cache_get(&(token, Some(holder))) {
                tracing::trace!(?token, ?holder, "cache hit (holder-specific strategy)");
                return strategy.clone();
            }
        }

        let strategy = detector.detect(token, holder).await;

        // Only cache when we successfully detect the token, or we can't find
        // it. Anything else is likely a temporary simulator (i.e. node) failure
        // which we don't want to cache.
        if matches!(&strategy, Ok(_) | Err(DetectionError::NotFound)) {
            tracing::debug!(?token, ?strategy, "caching auto-detected strategy");
            if let Ok(strategy) = strategy.as_ref() {
                let cache_key = (
                    token,
                    (!strategy.is_valid_for_all_holders()).then_some(holder),
                );
                cache
                    .lock()
                    .unwrap()
                    .cache_set(cache_key, Some(strategy.clone()));
            } else {
                // strategy is Err(DetectionError::NotFound)
                cache.lock().unwrap().cache_set((token, Some(holder)), None);
            }
        } else {
            tracing::warn!(
                ?token,
                ?strategy,
                "error auto-detecting token balance override strategy"
            );
        }

        strategy.ok()
    }
}

#[async_trait::async_trait]
impl BalanceOverriding for BalanceOverrides {
    async fn state_override(
        &self,
        request: BalanceOverrideRequest,
    ) -> Option<(Address, AccountOverride)> {
        let strategy = if let Some(strategy) = self.hardcoded.get(&request.token) {
            tracing::trace!(token = ?request.token, "using pre-configured balance override strategy");
            Some(strategy.clone())
        } else {
            self.cached_detection(request.token, request.holder).await
        }?;

        strategy
            .state_override(self.web3.as_ref(), &request.holder, &request.amount)
            .await
            .into_iter()
            .last()
    }
}

/// Balance overrider that always returns `None`. That can be
/// useful for testing.
pub struct DummyOverrider;

#[async_trait::async_trait]
impl BalanceOverriding for DummyOverrider {
    async fn state_override(
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
        crate::aave::{pack_user_state, ray_div},
        alloy_primitives::{address, b256},
        ethrpc::mock,
        maplit::hashmap,
    };

    #[tokio::test]
    async fn balance_override_computation() {
        let cow = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let balance_overrides = BalanceOverrides {
            hardcoded: hashmap! {
                cow => Strategy::SolidityMapping {
                    target_contract: cow,
                    map_slot: U256::from(0),
                },
            },
            ..Default::default()
        };

        assert_eq!(
            balance_overrides
                .state_override(BalanceOverrideRequest {
                    token: cow,
                    holder: address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
                    amount: U256::from(0x42),
                })
                .await,
            Some((
                cow,
                AccountOverride {
                    state_diff: Some(
                        iter::once((
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

        // You can verify the state override computation is correct by running:
        // ```
        // curl -X POST $RPC -H 'Content-Type: application/json' --data '{
        //   "jsonrpc": "2.0",
        //   "id": 0,
        //   "method": "eth_call",
        //   "params": [
        //     {
        //       "to": "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB",
        //       "data": "0x70a08231000000000000000000000000d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
        //     },
        //     "latest",
        //     {
        //       "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
        //         "stateDiff": {
        //           "0xfca351f4d96129454cfc8ef7930b638ac71fea35eb69ee3b8d959496beb04a33":
        //             "0x0000000000000000000000000000000000000000000000000000000000000042"
        //         }
        //       }
        //     }
        //   ]
        // }'
        // ```
    }

    #[tokio::test]
    async fn balance_overrides_none_for_unknown_tokens() {
        let balance_overrides = BalanceOverrides::default();
        assert_eq!(
            balance_overrides
                .state_override(BalanceOverrideRequest {
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
        let balance_overrides = BalanceOverrides {
            hardcoded: hashmap! {
                token => Strategy::SoladyMapping { target_contract: address!("0000000000c5dc95539589fbd24be07c6c14eca4") },
            },
            ..Default::default()
        };

        assert_eq!(
            balance_overrides
                .state_override(BalanceOverrideRequest {
                    token,
                    holder: address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
                    amount: U256::from(0x42),
                })
                .await,
            Some((
                token,
                AccountOverride {
                    state_diff: Some({
                        iter::once((
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

        // You can verify the state override computation is correct by running:
        // ```
        // curl -X POST $RPC -H 'Content-Type: application/json' --data '{
        //   "jsonrpc": "2.0",
        //   "id": 0,
        //   "method": "eth_call",
        //   "params": [
        //     {
        //       "to": "0x0000000000c5dc95539589fbd24be07c6c14eca4",
        //       "data": "0x70a08231000000000000000000000000d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
        //     },
        //     "latest",
        //     {
        //       "0x0000000000c5dc95539589fbd24be07c6c14eca4": {
        //         "stateDiff": {
        //           "f6a6656ed2d14bad3cdd3e8871db3f535a136a1b6cd5ae2dced8eb813f3d4e4f":
        //             "0x0000000000000000000000000000000000000000000000000000000000000042"
        //         }
        //       }
        //     }
        //   ]
        // }'
        // ```
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

        // Create a mock web3 and convert it to the expected type
        let mock_web3 = mock::web3();
        let balance_overrides = BalanceOverrides {
            hardcoded: Default::default(),
            detector: Some((
                Detector::new(
                    mock_web3.clone(),
                    60,
                    detector::DEFAULT_VERIFICATION_TIMEOUT,
                ),
                Mutex::new(SizedCache::with_size(100)),
            )),
            web3: Some(mock_web3),
        };

        // Manually populate the cache as if detector found this holder-agnostic
        // strategy
        {
            let (_, cache) = balance_overrides.detector.as_ref().unwrap();
            cache
                .lock()
                .unwrap()
                .cache_set((token, None), Some(strategy.clone()));
        }

        // Both holders should retrieve the same cached strategy since it's valid for
        // all holders
        assert_eq!(
            balance_overrides.cached_detection(token, holder1).await,
            Some(strategy.clone())
        );
        assert_eq!(
            balance_overrides.cached_detection(token, holder2).await,
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

        // Create a mock web3 and convert it to the expected type
        let mock_web3 = mock::web3();
        let balance_overrides = BalanceOverrides {
            hardcoded: Default::default(),
            detector: Some((
                Detector::new(
                    mock_web3.clone(),
                    60,
                    detector::DEFAULT_VERIFICATION_TIMEOUT,
                ),
                Mutex::new(SizedCache::with_size(100)),
            )),
            web3: Some(mock_web3),
        };

        // Manually populate cache with holder-specific strategies
        {
            let (_, cache) = balance_overrides.detector.as_ref().unwrap();
            cache
                .lock()
                .unwrap()
                .cache_set((token, Some(holder1)), Some(strategy_h1.clone()));
            cache
                .lock()
                .unwrap()
                .cache_set((token, Some(holder2)), Some(strategy_h2.clone()));
        }

        // Each holder should retrieve their specific cached strategy
        assert_eq!(
            balance_overrides.cached_detection(token, holder1).await,
            Some(strategy_h1)
        );
        assert_eq!(
            balance_overrides.cached_detection(token, holder2).await,
            Some(strategy_h2)
        );
    }

    #[test]
    fn ray_div_edge_cases() {
        let index = U256::from_str_radix("1063000000000000000000000000", 10).unwrap();
        // 0 / x = 0.
        assert_eq!(ray_div(U256::ZERO, index).unwrap(), U256::ZERO);
        // Divide by zero returns None.
        assert_eq!(
            ray_div(U256::from(1_000_000_000_000_000_000u128), U256::ZERO),
            None,
        );
    }

    #[test]
    fn pack_user_state_leaves_additional_data_intact() {
        let balance = U256::from(0x1234_5678u64);
        let extra = U256::from(0xabcd_ef01u64);
        let packed = pack_user_state(balance, extra);
        let word = U256::from_be_bytes(packed.0);

        let mask = (U256::from(1u64) << 128) - U256::from(1u64);
        assert_eq!(word & mask, balance);
        assert_eq!(word >> 128, extra);
    }

    #[test]
    fn pack_user_state_truncates_to_uint128() {
        // A value larger than uint128 should be masked down to its low 128
        // bits. Using 2^128 + 7 so we can check the wrap visibly.
        let overflow = (U256::from(1u64) << 128) + U256::from(7u64);
        let packed = pack_user_state(overflow, U256::ZERO);
        let word = U256::from_be_bytes(packed.0);
        assert_eq!(word, U256::from(7u64));
    }

    #[test]
    fn mapping_slot_hash_matches_solidity_layout() {
        // keccak256(pad32(holder) || map_slot) — verified with
        // `cast keccak $(cast abi-encode "f(address,uint256)" $HOLDER 52)`.
        let holder = address!("18709E89BD403F470088aBDAcEbE86CC60dda12e");
        let slot = mapping_slot_hash(&holder, &U256::from(52).to_be_bytes::<32>());
        assert_eq!(
            slot,
            b256!("6785743a4ad9de6e692f819936c9d0b94b199ed36f2660e82404737b769718e5")
        );
    }

    #[tokio::test]
    async fn aave_v3_a_token_override_scales_amount_and_writes_low_128() {
        use alloy_provider::mock::Asserter;

        // aEthWETH mainnet triple.
        let a_token = address!("4d5F47FA6A74757f35C14fD3a6Ef8E3C9BC514E8");
        let pool = address!("87870bca3f3fd6335c3f4ce8392d69350b4fa4e2");
        let underlying = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let holder = address!("18709E89BD403F470088aBDAcEbE86CC60dda12e");
        let amount = U256::from(1_000_000_000_000_000_000u128); // 1 aEthWETH

        let asserter = Asserter::new();
        // The mock responds to our `eth_call` with the encoded `uint256`
        // normalized income — a ray value just above 1.063.
        let index = U256::from_str_radix("1063000000000000000000000000", 10).unwrap();
        asserter.push_success(&format!("0x{:064x}", index));

        let balance_overrides = BalanceOverrides {
            hardcoded: hashmap! {
                a_token => Strategy::AaveV3AToken {
                    target_contract: a_token,
                    pool,
                    underlying,
                },
            },
            detector: None,
            web3: Some(Web3::with_asserter(asserter)),
        };

        let (addr, override_) = balance_overrides
            .state_override(BalanceOverrideRequest {
                token: a_token,
                holder,
                amount,
            })
            .await
            .expect("override computed");

        assert_eq!(addr, a_token);

        let diff = override_.state_diff.expect("state diff present");
        assert_eq!(diff.len(), 1);
        let (slot, value) = diff.into_iter().next().unwrap();
        // Slot is keccak256(holder || 52) — same one used by aEthWETH.
        assert_eq!(
            slot,
            b256!("6785743a4ad9de6e692f819936c9d0b94b199ed36f2660e82404737b769718e5")
        );
        // Scaled balance in low 128, zero in high 128 (safe for fresh
        // holders like the spardose).
        let word = U256::from_be_bytes(value.0);
        assert_eq!(word >> 128, U256::ZERO);
        assert_eq!(word, ray_div(amount, index).unwrap());
    }

    /// When no detector is configured, there's no `Web3` handle available
    /// and the `AaveV3AToken` resolver must cleanly fail rather than panic.
    #[tokio::test]
    async fn aave_v3_a_token_override_none_without_web3() {
        let a_token = address!("4d5F47FA6A74757f35C14fD3a6Ef8E3C9BC514E8");
        let balance_overrides = BalanceOverrides {
            hardcoded: hashmap! {
                a_token => Strategy::AaveV3AToken {
                    target_contract: a_token,
                    pool: address!("87870bca3f3fd6335c3f4ce8392d69350b4fa4e2"),
                    underlying: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                },
            },
            detector: None,
            web3: None,
        };

        let result = balance_overrides
            .state_override(BalanceOverrideRequest {
                token: a_token,
                holder: address!("18709E89BD403F470088aBDAcEbE86CC60dda12e"),
                amount: U256::from(1u64),
            })
            .await;
        assert!(result.is_none());
    }
}
