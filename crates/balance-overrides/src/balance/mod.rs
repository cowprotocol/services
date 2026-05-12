pub(crate) mod aave;

use {
    crate::detector::{DetectionError, SimulationError, extract_sload_slots, mapping_slot_hash},
    alloy_eips::BlockId,
    alloy_primitives::{Address, B256, TxKind, U256, keccak256, map::AddressMap},
    alloy_provider::ext::DebugApi,
    alloy_rpc_types::{
        TransactionInput,
        TransactionRequest,
        state::AccountOverride,
        trace::geth::GethDebugTracingCallOptions,
    },
    alloy_sol_types::SolCall,
    alloy_transport::TransportErrorKind,
    cached::{Cached, SizedCache},
    contracts::ERC20,
    ethrpc::Web3,
    std::{collections::HashMap, iter, sync::Mutex, time::Duration},
};

/// These are the solady magic bytes for user balances
/// https://github.com/Vectorized/solady/blob/main/src/tokens/ERC20.sol#L81
const BALANCE_SLOT_SEED: &[u8] = &[0x87, 0xa2, 0x11, 0xa2];

/// Used by Detector when there are multiple slots to increase the chances we
/// find the correct storage slot quickly.
///
/// Note: does not emit `AaveV3AToken` candidates — Aave is handled by the
/// fast-path in `Detector::detect_uncached` which tries the canonical
/// `_userState` slot directly.
pub(crate) fn find_plausible_strategies_for_slots(
    storage_slots: &[(Address, B256)],
    holder: &Address,
    heuristic_depth: usize,
) -> Vec<Strategy> {
    // Build a map from heuristic slot hash to the map_slot index
    let mut solidity_mapping_slot_to_index = HashMap::new();

    let mut buf = [0; 64];
    buf[12..32].copy_from_slice(holder.as_slice());
    for i in 0..heuristic_depth {
        buf[32..64].copy_from_slice(&U256::from(i).to_be_bytes::<32>());
        let slot_hash = keccak256(buf);
        solidity_mapping_slot_to_index.insert(slot_hash, i);
    }

    buf[0..20].copy_from_slice(holder.as_slice());
    buf[20..28].copy_from_slice(&[0x0; 8]); // zeroize dirtied section of buffer
    buf[28..32].copy_from_slice(BALANCE_SLOT_SEED);
    let solady_slot = keccak256(&buf[0..32]);

    let mut heuristic_strategies = Vec::new();
    let mut fallback_strategies = Vec::new();
    for (contract, slot) in storage_slots.iter().rev() {
        if let Some(&map_slot_index) = solidity_mapping_slot_to_index.get(slot) {
            heuristic_strategies.push(Strategy::SolidityMapping {
                target_contract: *contract,
                map_slot: U256::from(map_slot_index),
            });
        } else if *slot == solady_slot {
            heuristic_strategies.push(Strategy::SoladyMapping {
                target_contract: *contract,
            });
        } else {
            fallback_strategies.push(Strategy::DirectSlot {
                target_contract: *contract,
                slot: *slot,
            });
        };
    }

    heuristic_strategies.extend(fallback_strategies);
    heuristic_strategies
}

/// Parameters for computing a balance state override.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct BalanceOverrideRequest {
    /// The token for the override.
    pub token: Address,
    /// The account to override the balance for.
    pub holder: Address,
    /// The token amount (in atoms) to set the balance to.
    pub amount: U256,
}

/// Resolved balance override strategy.
///
/// The `AaveV3AToken` variant owns a cloned `Web3` handle so it can compute
/// the override fully autonomously — no external web3 reference needs to be
/// threaded through.
#[derive(Clone, Debug)]
pub(crate) enum Strategy {
    SolidityMapping {
        target_contract: Address,
        map_slot: U256,
    },
    SoladyMapping {
        target_contract: Address,
    },
    DirectSlot {
        target_contract: Address,
        slot: B256,
    },
    AaveV3AToken {
        target_contract: Address,
        pool: Address,
        underlying: Address,
        web3: Web3,
    },
}

impl Strategy {
    pub(crate) async fn state_override(
        &self,
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
                web3,
            } => {
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

    /// Returns whether the strategy can cheaply compute the necessary state
    /// override for any given holder or if it only works for the original
    /// holder it was generated for.
    pub(crate) fn can_be_applied_to_any_holder(&self) -> bool {
        !matches!(self, Self::DirectSlot { .. })
    }
}

/// Compare by addresses only; `web3` is intentionally excluded since two
/// strategies for the same token/pool/underlying are semantically equivalent
/// regardless of which web3 handle they carry.
impl PartialEq for Strategy {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::SolidityMapping {
                    target_contract: tc1,
                    map_slot: ms1,
                },
                Self::SolidityMapping {
                    target_contract: tc2,
                    map_slot: ms2,
                },
            ) => tc1 == tc2 && ms1 == ms2,
            (
                Self::SoladyMapping {
                    target_contract: tc1,
                },
                Self::SoladyMapping {
                    target_contract: tc2,
                },
            ) => tc1 == tc2,
            (
                Self::DirectSlot {
                    target_contract: tc1,
                    slot: s1,
                },
                Self::DirectSlot {
                    target_contract: tc2,
                    slot: s2,
                },
            ) => tc1 == tc2 && s1 == s2,
            (
                Self::AaveV3AToken {
                    target_contract: tc1,
                    pool: p1,
                    underlying: u1,
                    ..
                },
                Self::AaveV3AToken {
                    target_contract: tc2,
                    pool: p2,
                    underlying: u2,
                    ..
                },
            ) => tc1 == tc2 && p1 == p2 && u1 == u2,
            _ => false,
        }
    }
}

impl Eq for Strategy {}

type Cache = SizedCache<(Address, Option<Address>), Option<Strategy>>;

/// Heuristic balance override detector with integrated caching.
///
/// Owns the Web3 handle, detection parameters, and the per-token strategy
/// cache. `AaveV3AToken` strategies in the cache carry a cloned `Web3` handle
/// so they can compute overrides without any external dependency.
pub(crate) struct Detector {
    web3: Web3,
    probing_depth: u8,
    verification_timeout: Duration,
    pub(crate) cache: Mutex<Cache>,
}

impl Detector {
    pub fn new(
        web3: Web3,
        probing_depth: u8,
        verification_timeout: Duration,
        cache_size: usize,
    ) -> Self {
        Self {
            web3,
            probing_depth,
            verification_timeout,
            cache: Mutex::new(SizedCache::with_size(cache_size)),
        }
    }

    /// Returns the cached detection result for `(token, holder)`, running
    /// detection if not yet cached.
    pub async fn detect(&self, token: Address, holder: Address) -> Option<Strategy> {
        tracing::trace!(?token, "attempting to auto-detect balance slot");

        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(strategy) = cache.cache_get(&(token, None)) {
                tracing::trace!(?token, "cache hit (strategy valid for all holders)");
                return strategy.clone();
            }
            if let Some(strategy) = cache.cache_get(&(token, Some(holder))) {
                tracing::trace!(?token, ?holder, "cache hit (holder-specific strategy)");
                return strategy.clone();
            }
        }

        let strategy = self.detect_uncached(token, holder).await;

        if matches!(&strategy, Ok(_) | Err(DetectionError::NotFound)) {
            tracing::debug!(?token, ?strategy, "caching auto-detected balance strategy");
            if let Ok(strategy) = strategy.as_ref() {
                let cache_key = (
                    token,
                    (!strategy.can_be_applied_to_any_holder()).then_some(holder),
                );
                self.cache
                    .lock()
                    .unwrap()
                    .cache_set(cache_key, Some(strategy.clone()));
            } else {
                self.cache
                    .lock()
                    .unwrap()
                    .cache_set((token, Some(holder)), None);
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

    async fn detect_uncached(
        &self,
        token: Address,
        holder: Address,
    ) -> Result<Strategy, DetectionError<TransportErrorKind>> {
        // Aave fast-path. If the token self-identifies as a v3 aToken and
        // the pool confirms it, try the canonical `_userState` slot
        // directly — no `debug_traceCall` needed. An Aave v3 fork that
        // moved `_userState` to a different slot won't verify here and
        // will fall through to the generic trace-based path, which only
        // ever returns non-Aave strategies; such a fork needs an explicit
        // hardcoded config entry.
        if let Some((pool, underlying)) = aave::probe_aave_token(&self.web3, token).await {
            let candidate = Strategy::AaveV3AToken {
                target_contract: token,
                pool,
                underlying,
                web3: self.web3.clone(),
            };
            if self
                .verify_strategy(token, holder, &candidate)
                .await
                .is_ok()
            {
                tracing::debug!(?token, "detected Aave v3 aToken");
                return Ok(candidate);
            }
            tracing::debug!(
                ?token,
                "Aave probe succeeded but canonical slot didn't verify; falling back to trace"
            );
        }

        let balance_of_call = ERC20::ERC20::balanceOfCall { account: holder };
        let calldata = balance_of_call.abi_encode();

        let call_request = TransactionRequest {
            to: Some(TxKind::Call(token)),
            input: TransactionInput::new(calldata.into()),
            ..Default::default()
        };

        let trace = self
            .web3
            .provider
            .debug_trace_call(
                call_request,
                BlockId::latest(),
                GethDebugTracingCallOptions::default(),
            )
            .await
            .map_err(|err| {
                tracing::debug!(?token, ?err, "debug_traceCall not supported for token");
                DetectionError::Rpc(err)
            })?;

        let storage_slots = extract_sload_slots(trace, token);

        if storage_slots.is_empty() {
            tracing::debug!("no SLOAD operations found in trace for token {:?}", token);
            return Err(DetectionError::NotFound);
        }

        let strategies =
            find_plausible_strategies_for_slots(&storage_slots, &holder, self.probing_depth.into());

        if strategies.len() == 1 {
            let slot = storage_slots[0];
            tracing::debug!(
                storage_context = ?slot.0,
                slot = ?slot.1,
                ?slot,
                iterations = 0,
                "detected balance slot via trace (single SLOAD) for token",
            );
            return Ok(strategies.into_iter().next().unwrap());
        }

        tracing::debug!(
            ?token,
            total = storage_slots.len(),
            "multiple SLOAD operations, testing each one",
        );

        for (i, strategy) in strategies
            .iter()
            .take(self.probing_depth.into())
            .enumerate()
        {
            // Some tokens (e.g. reflection tokens like LuckyBlock) have
            // `balanceOf` implementations that iterate over storage arrays.
            // During verification we override storage slots with a test value —
            // if that value lands on an array-length slot the EVM loops until
            // the node's execution timeout. A per-strategy timeout prevents one
            // slow slot from blocking the entire detection.
            let result = tokio::time::timeout(
                self.verification_timeout,
                self.verify_strategy(token, holder, strategy),
            )
            .await;

            match result {
                Ok(Ok(())) => {
                    tracing::debug!(
                        ?token,
                        ?holder,
                        ?strategy,
                        iterations = i + 1,
                        total = storage_slots.len(),
                        "verified balance strategy via testing",
                    );
                    return Ok(strategy.clone());
                }
                Err(_) => {
                    tracing::warn!(
                        ?token,
                        ?holder,
                        ?strategy,
                        "balance override strategy verification timed out, skipping",
                    );
                }
                Ok(Err(_)) => {}
            }
        }

        tracing::debug!(
            "none of the SLOAD slots appear to be the balance slot for token {:?}",
            token
        );

        Err(DetectionError::NotFound)
    }

    async fn verify_strategy(
        &self,
        token: Address,
        holder: Address,
        strategy: &Strategy,
    ) -> Result<(), DetectionError<TransportErrorKind>> {
        let test_balance = U256::from(0x1337_1337_1337_1337_u64);

        let overrides = strategy.state_override(&holder, &test_balance).await;
        if overrides.is_empty() {
            return Err(DetectionError::NotFound);
        }

        let token_contract = ERC20::Instance::new(token, self.web3.provider.clone());
        let balance = token_contract
            .balanceOf(holder)
            .state(overrides)
            .call()
            .await
            .map_err(|e| {
                DetectionError::Simulation(SimulationError::Other(anyhow::anyhow!(
                    "balanceOf call failed during strategy verification: {e}"
                )))
            })?;

        verified_balance_matches(strategy, balance, test_balance)
            .then_some(())
            .ok_or(DetectionError::NotFound)
    }
}

impl std::fmt::Debug for Detector {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("balance::Detector")
            .field("probing_depth", &self.probing_depth)
            .field("verification_timeout", &self.verification_timeout)
            .finish()
    }
}

fn verified_balance_matches(strategy: &Strategy, balance: U256, test_balance: U256) -> bool {
    match strategy {
        Strategy::AaveV3AToken { .. } => balance.abs_diff(test_balance) <= U256::ONE,
        _ => balance == test_balance,
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy_primitives::address};

    #[test]
    fn test_create_strategies_reverses_order() {
        let contract = Address::with_last_byte(2);
        let contract2 = Address::with_last_byte(2);
        let slot1 = B256::with_last_byte(1);
        let slot2 = B256::with_last_byte(2);
        let slot3 = B256::with_last_byte(3);
        let slots = vec![
            (contract, slot1),
            (contract2, slot3),
            (contract, slot2),
            (contract, slot3),
            (contract2, slot1),
        ];
        let holder = Address::with_last_byte(1);

        let strategies = find_plausible_strategies_for_slots(&slots, &holder, 5);

        assert_eq!(
            strategies,
            vec![
                Strategy::DirectSlot {
                    target_contract: contract2,
                    slot: slot1,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot3,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot2,
                },
                Strategy::DirectSlot {
                    target_contract: contract2,
                    slot: slot3,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot1,
                },
            ]
        );
    }

    #[test]
    fn test_create_strategies_with_heuristic_slots_stable() {
        let contract = Address::with_last_byte(2);
        let holder = address!("1111111111111111111111111111111111111111");

        let mut buf = [0u8; 64];
        buf[12..32].copy_from_slice(holder.as_slice());
        buf[32..64].copy_from_slice(&U256::ZERO.to_be_bytes::<32>());
        let heuristic_slot1 = keccak256(buf);
        buf[32..64].copy_from_slice(&U256::from(5).to_be_bytes::<32>());
        let heuristic_slot2 = keccak256(buf);
        buf[0..20].copy_from_slice(holder.as_slice());
        buf[20..28].copy_from_slice(&[0x0; 8]);
        buf[28..32].copy_from_slice(BALANCE_SLOT_SEED);
        let heuristic_slot3 = keccak256(&buf[0..32]);

        let slot1 = B256::with_last_byte(0xe7);
        let slot2 = B256::with_last_byte(0xe6);
        let slot3 = B256::with_last_byte(0xe3);

        let slots = vec![
            (contract, slot1),
            (contract, slot3),
            (contract, heuristic_slot2),
            (contract, heuristic_slot3),
            (contract, slot2),
            (contract, heuristic_slot1),
        ];

        let strategies = find_plausible_strategies_for_slots(&slots, &holder, 100);

        assert_eq!(
            strategies,
            vec![
                Strategy::SolidityMapping {
                    target_contract: contract,
                    map_slot: U256::ZERO,
                },
                Strategy::SoladyMapping {
                    target_contract: contract,
                },
                Strategy::SolidityMapping {
                    target_contract: contract,
                    map_slot: U256::from(5),
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot2,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot3,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot1,
                },
            ]
        );
    }

    #[test]
    fn test_create_strategies_zero_heuristic_depth() {
        let contract = Address::with_last_byte(2);
        let holder = address!("5555555555555555555555555555555555555555");

        let slot1 = B256::with_last_byte(1);
        let slot2 = B256::with_last_byte(2);
        let slot3 = B256::with_last_byte(3);

        let mut buf = [0u8; 64];
        buf[12..32].copy_from_slice(holder.as_slice());
        buf[32..64].copy_from_slice(&U256::ZERO.to_be_bytes::<32>());
        let heuristic_slot = keccak256(buf);

        let slots = vec![
            (contract, slot1),
            (contract, slot2),
            (contract, heuristic_slot),
            (contract, slot3),
        ];

        let strategies = find_plausible_strategies_for_slots(&slots, &holder, 0);

        assert_eq!(
            strategies,
            vec![
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot3,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: heuristic_slot,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot2,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot1,
                },
            ]
        );
    }

    const OPEN_ZEPPELIN_ERC20_UPGRADEABLE: B256 =
        alloy_primitives::b256!("52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00");

    #[ignore]
    #[tokio::test]
    async fn detects_storage_slots_mainnet() {
        use crate::detector::DEFAULT_VERIFICATION_TIMEOUT;

        let detector = Detector::new(Web3::new_from_env(), 60, DEFAULT_VERIFICATION_TIMEOUT, 100);

        let storage = detector
            .detect(
                address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                Address::with_last_byte(1),
            )
            .await
            .unwrap();
        assert_eq!(
            storage,
            Strategy::SolidityMapping {
                target_contract: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                map_slot: U256::from(3)
            }
        );

        let storage = detector
            .detect(
                address!("4956b52ae2ff65d74ca2d61207523288e4528f96"),
                Address::with_last_byte(1),
            )
            .await
            .unwrap();
        assert_eq!(
            storage,
            Strategy::SolidityMapping {
                target_contract: address!("4956b52ae2ff65d74ca2d61207523288e4528f96"),
                map_slot: <U256 as From<_>>::from(OPEN_ZEPPELIN_ERC20_UPGRADEABLE),
            }
        );

        let storage = detector
            .detect(
                address!("0000000000c5dc95539589fbd24be07c6c14eca4"),
                Address::with_last_byte(1),
            )
            .await
            .unwrap();
        assert_eq!(
            storage,
            Strategy::SoladyMapping {
                target_contract: address!("0000000000c5dc95539589fbd24be07c6c14eca4")
            }
        );
    }

    #[ignore]
    #[tokio::test]
    async fn detects_storage_slots_arbitrum() {
        use crate::detector::DEFAULT_VERIFICATION_TIMEOUT;

        let detector = Detector::new(Web3::new_from_env(), 60, DEFAULT_VERIFICATION_TIMEOUT, 100);

        let storage = detector
            .detect(
                address!("ff970a61a04b1ca14834a43f5de4533ebddb5cc8"),
                Address::with_last_byte(1),
            )
            .await
            .unwrap();
        assert_eq!(
            storage,
            Strategy::SolidityMapping {
                target_contract: address!("ff970a61a04b1ca14834a43f5de4533ebddb5cc8"),
                map_slot: U256::from(51)
            }
        );
    }
}
