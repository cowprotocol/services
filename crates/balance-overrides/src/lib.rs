pub mod detector;

use {
    self::detector::{DetectionError, Detector},
    alloy_eips::BlockId,
    alloy_primitives::{Address, B256, Bytes, TxKind, U256, keccak256, map::AddressMap},
    alloy_provider::Provider,
    alloy_rpc_types::{TransactionInput, TransactionRequest, state::AccountOverride},
    alloy_sol_types::{SolCall, sol},
    cached::{Cached, SizedCache},
    configs::balance_overrides::Strategy,
    ethrpc::Web3,
    std::{collections::HashMap, iter, sync::Mutex},
};

sol! {
    /// Minimal interface for the Aave v3 `Pool` used to derive the current
    /// liquidity index applied by aTokens when reporting `balanceOf`.
    interface IAaveV3Pool {
        function getReserveNormalizedIncome(address asset) external view returns (uint256);
    }
}

/// Ray (1e27) is Aave's 27-decimal fixed-point unit used in `rayDiv`.
const RAY: U256 = U256::from_limbs([0x9fd0803ce8000000, 0x33b2e3c, 0, 0]);
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

trait StrategyExt {
    /// Computes the storage slot and value to override for a particular token
    /// holder and amount.
    fn state_override(&self, holder: &Address, amount: &U256) -> AddressMap<AccountOverride>;

    fn is_valid_for_all_holders(&self) -> bool;
}

impl StrategyExt for Strategy {
    fn state_override(&self, holder: &Address, amount: &U256) -> AddressMap<AccountOverride> {
        let (target_contract, key) = match self {
            Self::SolidityMapping {
                target_contract,
                map_slot,
            } => (
                target_contract,
                mapping_slot_hash(holder, &map_slot.to_be_bytes::<32>()),
            ),
            Self::SoladyMapping { target_contract } => {
                let mut buf = [0; 32];
                buf[0..20].copy_from_slice(holder.as_slice());
                buf[28..32].copy_from_slice(&[0x87, 0xa2, 0x11, 0xa2]);
                (target_contract, keccak256(buf))
            }
            Self::DirectSlot {
                target_contract,
                slot,
            } => (target_contract, *slot),
            // AaveV3AToken requires an async call to fetch the current
            // normalized income, so it is handled separately in
            // `BalanceOverrides::state_override`.
            Self::AaveV3AToken { .. } => unreachable!(
                "AaveV3AToken strategy must be resolved asynchronously, not via StrategyExt"
            ),
        };

        let state_override = AccountOverride {
            state_diff: Some(iter::once((key, B256::new(amount.to_be_bytes::<32>()))).collect()),
            ..Default::default()
        };

        iter::once((*target_contract, state_override)).collect()
    }

    fn is_valid_for_all_holders(&self) -> bool {
        matches!(self, Self::DirectSlot { .. })
    }
}

/// Computes `keccak256(pad32(holder) ++ map_slot)` — the storage slot of
/// `mapping(address => _)` entries in Solidity.
fn mapping_slot_hash(holder: &Address, map_slot: &[u8; 32]) -> B256 {
    let mut buf = [0u8; 64];
    buf[12..32].copy_from_slice(holder.as_slice());
    buf[32..64].copy_from_slice(map_slot);
    keccak256(buf)
}

/// Packs a `UserState { uint128 balance; uint128 additionalData }` into a
/// 32-byte word. The balance occupies the lower 128 bits; `additional_data`
/// sits in the upper 128 bits.
fn pack_user_state(balance: U256, additional_data: U256) -> B256 {
    let mask = (U256::from(1u64) << 128) - U256::from(1u64);
    let packed: U256 = ((additional_data & mask) << 128) | (balance & mask);
    B256::new(packed.to_be_bytes::<32>())
}

/// Ray-division: `(a * RAY + b/2) / b`, round-half-up. This matches Aave's
/// `WadRayMath.rayDiv` bit-for-bit so the scaled amount we write into
/// storage equals the one Aave will itself compute during a subsequent
/// `_transfer`. Returns `None` if `b == 0` or the intermediate product
/// overflows `U256`.
fn ray_div(a: U256, b: U256) -> Option<U256> {
    if b.is_zero() {
        return None;
    }
    let half_b = b >> 1;
    a.checked_mul(RAY)
        .and_then(|prod| prod.checked_add(half_b))
        .map(|num| num / b)
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
    /// auto-detection is not enabled.
    pub detector: Option<(Detector, DetectorCache)>,
    /// Optional web3 handle used by strategies that need runtime on-chain
    /// reads (e.g. `AaveV3AToken` fetching the reserve's normalized income).
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

        if let Strategy::AaveV3AToken {
            target_contract,
            pool,
            underlying,
            map_slot,
        } = &strategy
        {
            return self
                .aave_v3_a_token_override(
                    *target_contract,
                    *pool,
                    *underlying,
                    *map_slot,
                    request.holder,
                    request.amount,
                )
                .await;
        }

        strategy
            .state_override(&request.holder, &request.amount)
            .into_iter()
            .last()
    }
}

impl BalanceOverrides {
    /// Resolves an `AaveV3AToken` balance override. We need the current
    /// normalized income from the Aave pool to invert the scaling applied by
    /// aToken `balanceOf`. Writes the scaled amount into the low 128 bits of
    /// the packed `UserState` slot; the upper 128 bits (`additionalData`) are
    /// zeroed, which is safe for fresh holders like the spardose.
    async fn aave_v3_a_token_override(
        &self,
        a_token: Address,
        pool: Address,
        underlying: Address,
        map_slot: U256,
        holder: Address,
        amount: U256,
    ) -> Option<(Address, AccountOverride)> {
        let web3 = self.web3.as_ref().or_else(|| {
            tracing::warn!(
                ?a_token,
                "AaveV3AToken balance override requested but web3 is not configured",
            );
            None
        })?;

        let call = IAaveV3Pool::getReserveNormalizedIncomeCall { asset: underlying };
        let calldata = Bytes::from(call.abi_encode());
        let tx = TransactionRequest {
            to: Some(TxKind::Call(pool)),
            input: TransactionInput::new(calldata),
            ..Default::default()
        };
        let index = match web3.provider.call(tx).block(BlockId::latest()).await {
            Ok(bytes) => {
                match IAaveV3Pool::getReserveNormalizedIncomeCall::abi_decode_returns(&bytes) {
                    Ok(index) => index,
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            ?pool,
                            ?underlying,
                            "failed to decode Aave reserve normalized income response"
                        );
                        return None;
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    ?pool,
                    ?underlying,
                    "failed to fetch Aave reserve normalized income"
                );
                return None;
            }
        };

        let scaled = ray_div(amount, index)?;
        let slot = mapping_slot_hash(&holder, &map_slot.to_be_bytes::<32>());
        let value = pack_user_state(scaled, U256::ZERO);

        tracing::trace!(
            ?a_token,
            ?holder,
            %amount,
            %index,
            %scaled,
            "computed AaveV3AToken balance override"
        );

        let state_override = AccountOverride {
            state_diff: Some(iter::once((slot, value)).collect()),
            ..Default::default()
        };
        Some((a_token, state_override))
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

    /// Round-half-up `rayMul` — the same formula aToken's `balanceOf` applies
    /// to convert scaled storage into the reported display balance. Only used
    /// by the bug-reproduction test below.
    fn ray_mul(a: U256, b: U256) -> U256 {
        (a * b + (RAY >> 1)) / RAY
    }

    /// Reproduction of the aEthWETH bug in pure math: without the
    /// `AaveV3AToken` strategy we would write the raw display `amount` into
    /// the balance slot, and aToken's `balanceOf` would then return
    /// `rayMul(amount, index)` — which differs from `amount` as soon as the
    /// reserve has accrued any interest (`index > RAY`). The `AaveV3AToken`
    /// strategy writes `rayDiv(amount, index)` instead, which round-trips
    /// back to `amount` within one wei of ray rounding.
    #[test]
    fn a_token_balance_override_bug_reproduction() {
        let amount = U256::from(1_000_000_000_000_000_000u128); // 1 aEthWETH
        // Mainnet aEthWETH normalized-income ~1.0632 RAY (observed today).
        let index = U256::from_str_radix("1063211170513245730547525051", 10).unwrap();
        assert!(
            index > RAY,
            "index must include accrued interest to reproduce"
        );

        // OLD behaviour (SolidityMapping / SoladyMapping / DirectSlot all
        // write the raw value): balanceOf returns rayMul(amount, index), which
        // is materially larger than `amount`. That mismatch is why the
        // detector's `verify_strategy` fails and why, if we force it via
        // hardcoded config, the spardose has more scaled balance than the
        // trade needs and downstream Aave math still underflows.
        let old_reported = ray_mul(amount, index);
        assert!(
            old_reported > amount + U256::from(10_000u64),
            "bug not reproduced: old strategy only off by {} wei",
            old_reported - amount,
        );

        // NEW behaviour (AaveV3AToken): we write rayDiv(amount, index). The
        // round-trip through aToken's balanceOf is within one wei.
        let scaled = ray_div(amount, index).unwrap();
        let new_reported = ray_mul(scaled, index);
        let diff = if new_reported >= amount {
            new_reported - amount
        } else {
            amount - new_reported
        };
        assert!(
            diff <= U256::from(1u64),
            "new strategy off by {diff} wei (expected ≤ 1)",
        );
    }

    #[test]
    fn ray_div_edge_cases() {
        // Numerical correctness on realistic values is covered by
        // `a_token_balance_override_bug_reproduction` (round-trip through
        // `ray_mul`). Here we only pin down the two edge cases:

        // 0 / x = 0.
        let index = U256::from_str_radix("1063000000000000000000000000", 10).unwrap();
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
                    map_slot: U256::from(52),
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

    /// End-to-end test against real mainnet: computes an `AaveV3AToken`
    /// override for aEthWETH, then calls `balanceOf(holder)` against the real
    /// node with that override applied and checks the result matches the
    /// requested amount (within ray rounding). Set `NODE_URL` to a mainnet
    /// RPC endpoint.
    #[ignore]
    #[tokio::test]
    async fn aave_v3_a_token_override_mainnet_roundtrip() {
        use contracts::ERC20;

        let a_token = address!("4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8");
        let pool = address!("87870bca3f3fd6335c3f4ce8392d69350b4fa4e2");
        let underlying = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        // Spardose lookalike: a fresh holder with zero prior state.
        let holder = address!("0000000000000000000000000000000000020000");
        let amount = U256::from(5_000_000_000_000_000_000u128); // 5 aEthWETH

        let web3 = ethrpc::Web3::new_from_env();
        let balance_overrides = BalanceOverrides {
            hardcoded: hashmap! {
                a_token => Strategy::AaveV3AToken {
                    target_contract: a_token,
                    pool,
                    underlying,
                    map_slot: U256::from(52),
                },
            },
            detector: None,
            web3: Some(web3.clone()),
        };

        let (target, override_) = balance_overrides
            .state_override(BalanceOverrideRequest {
                token: a_token,
                holder,
                amount,
            })
            .await
            .expect("override computed");
        assert_eq!(target, a_token);

        // Apply the override to a live balanceOf call and make sure the
        // contract now reports the requested amount (± 1 wei ray rounding).
        let overrides: alloy_rpc_types::state::StateOverride =
            iter::once((target, override_)).collect();
        let token_contract = ERC20::Instance::new(a_token, web3.provider.clone());
        let reported = token_contract
            .balanceOf(holder)
            .state(overrides)
            .call()
            .await
            .unwrap();

        let diff = if reported > amount {
            reported - amount
        } else {
            amount - reported
        };
        assert!(
            diff <= U256::from(1u64),
            "balanceOf returned {reported}, expected ~{amount} (diff {diff} wei)",
        );
    }

    #[tokio::test]
    async fn aave_v3_a_token_override_none_without_web3() {
        let a_token = address!("4d5F47FA6A74757f35C14fD3a6Ef8E3C9BC514E8");
        let balance_overrides = BalanceOverrides {
            hardcoded: hashmap! {
                a_token => Strategy::AaveV3AToken {
                    target_contract: a_token,
                    pool: address!("87870bca3f3fd6335c3f4ce8392d69350b4fa4e2"),
                    underlying: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                    map_slot: U256::from(52),
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
