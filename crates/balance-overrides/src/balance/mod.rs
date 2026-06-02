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

/// Distinct-byte value used both to verify a balance override (write it, expect
/// `balanceOf` to return it) and to recover a packed balance's bit offset from
/// how far `balanceOf` shifts it down (see `detect_byte_shift`). The bytes are
/// distinct so the detected shift is unambiguous. It is kept to 64 bits so it
/// fits narrow balance fields (e.g. UNI and COMP store balances as `uint96`)
/// without being masked during verification, which would otherwise reject those
/// tokens. The 64-bit width caps detectable shifts at 56 bits, which covers
/// "small flag below the balance" packings like AUSD (`shift_bits == 8`).
const SENTINEL: u64 = 0x0102_0304_0506_0708;

/// Used by Detector when there are multiple slots to increase the chances we
/// find the correct storage slot quickly.
///
/// Note: does not emit `AaveV3AToken` candidates — Aave is handled by the
/// fast-path in `Detector::uncached_detect` which tries the canonical
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
                shift_bits: 0,
            });
        } else if *slot == solady_slot {
            heuristic_strategies.push(Strategy::SoladyMapping {
                target_contract: *contract,
                shift_bits: 0,
            });
        } else {
            fallback_strategies.push(Strategy::DirectSlot {
                target_contract: *contract,
                slot: *slot,
                shift_bits: 0,
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
/// `shift_bits` is how far the balance is left-shifted within its slot, for
/// tokens that pack the balance above lower-order fields. E.g. AUSD stores
/// `{ bool isFrozen; uint248 balance }` in one slot, so the balance is the high
/// 248 bits and `shift_bits == 8`. It is `0` for the common unpacked case.
///
/// The `AaveV3AToken` variant owns a cloned `Web3` handle so it can compute
/// the override fully autonomously — no external web3 reference needs to be
/// threaded through.
#[derive(Clone, Debug)]
pub(crate) enum Strategy {
    SolidityMapping {
        target_contract: Address,
        map_slot: U256,
        shift_bits: usize,
    },
    SoladyMapping {
        target_contract: Address,
        shift_bits: usize,
    },
    DirectSlot {
        target_contract: Address,
        slot: B256,
        shift_bits: usize,
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
        let (target_contract, key, shift_bits) = match self {
            Self::SolidityMapping {
                target_contract,
                map_slot,
                shift_bits,
            } => (
                *target_contract,
                mapping_slot_hash(holder, &map_slot.to_be_bytes()),
                *shift_bits,
            ),
            Self::SoladyMapping {
                target_contract,
                shift_bits,
            } => {
                let mut buf = [0; 32];
                buf[0..20].copy_from_slice(holder.as_slice());
                buf[28..32].copy_from_slice(&[0x87, 0xa2, 0x11, 0xa2]);
                (*target_contract, keccak256(buf), *shift_bits)
            }
            Self::DirectSlot {
                target_contract,
                slot,
                shift_bits,
            } => (*target_contract, *slot, *shift_bits),
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

        // Pack the balance into its slot position. `None` means it doesn't fit
        // at this bit offset, so there's nothing to override.
        let Some(value) = packed_value(*amount, shift_bits) else {
            return AddressMap::default();
        };

        let state_override = AccountOverride {
            state_diff: Some(iter::once((key, B256::new(value.to_be_bytes::<32>()))).collect()),
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

    /// Sets the packing shift on a balance strategy. No-op for `AaveV3AToken`,
    /// which is never packed.
    fn with_shift(mut self, shift_bits: usize) -> Self {
        match &mut self {
            Self::SolidityMapping { shift_bits: s, .. }
            | Self::SoladyMapping { shift_bits: s, .. }
            | Self::DirectSlot { shift_bits: s, .. } => *s = shift_bits,
            Self::AaveV3AToken { .. } => {}
        }
        self
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
                    shift_bits: sb1,
                },
                Self::SolidityMapping {
                    target_contract: tc2,
                    map_slot: ms2,
                    shift_bits: sb2,
                },
            ) => tc1 == tc2 && ms1 == ms2 && sb1 == sb2,
            (
                Self::SoladyMapping {
                    target_contract: tc1,
                    shift_bits: sb1,
                },
                Self::SoladyMapping {
                    target_contract: tc2,
                    shift_bits: sb2,
                },
            ) => tc1 == tc2 && sb1 == sb2,
            (
                Self::DirectSlot {
                    target_contract: tc1,
                    slot: s1,
                    shift_bits: sb1,
                },
                Self::DirectSlot {
                    target_contract: tc2,
                    slot: s2,
                    shift_bits: sb2,
                },
            ) => tc1 == tc2 && s1 == s2 && sb1 == sb2,
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

        let strategy = self.uncached_detect(token, holder).await;

        match strategy.as_ref() {
            Ok(strategy) => {
                let cache_key = (
                    token,
                    (!strategy.can_be_applied_to_any_holder()).then_some(holder),
                );
                tracing::debug!(?token, ?strategy, "caching auto-detected balance strategy");
                self.cache
                    .lock()
                    .unwrap()
                    .cache_set(cache_key, Some(strategy.clone()));
            }
            Err(DetectionError::NotFound) => {
                tracing::debug!(?token, "caching token as unsupported");
                self.cache
                    .lock()
                    .unwrap()
                    .cache_set((token, Some(holder)), None);
            }
            Err(err) => {
                tracing::warn!(
                    ?token,
                    ?strategy,
                    ?err,
                    "error auto-detecting token balance override strategy"
                );
            }
        }

        strategy.ok()
    }

    async fn uncached_detect(
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
            if let Ok(resolved) = self.verify_strategy(token, holder, candidate).await {
                tracing::debug!(?token, "detected Aave v3 aToken");
                return Ok(resolved);
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

        tracing::debug!(
            ?token,
            total = storage_slots.len(),
            "testing candidate balance slots",
        );

        for (i, strategy) in strategies.into_iter().enumerate() {
            // Some tokens (e.g. reflection tokens like LuckyBlock) have
            // `balanceOf` implementations that iterate over storage arrays.
            // During verification we override storage slots with a test value —
            // if that value lands on an array-length slot the EVM loops until
            // the node's execution timeout. A per-strategy timeout prevents one
            // slow slot from blocking the entire detection.
            let result = tokio::time::timeout(
                self.verification_timeout,
                self.verify_strategy(token, holder, strategy.clone()),
            )
            .await;

            match result {
                Ok(Ok(verified)) => {
                    tracing::debug!(
                        ?token,
                        ?holder,
                        strategy = ?verified,
                        iterations = i + 1,
                        total = storage_slots.len(),
                        "verified balance strategy via testing",
                    );
                    return Ok(verified);
                }
                Err(_) => {
                    tracing::warn!(
                        ?token,
                        ?holder,
                        ?strategy,
                        "balance override strategy verification timed out, skipping",
                    );
                }
                Ok(Err(err)) => {
                    tracing::trace!(
                        ?token,
                        ?holder,
                        ?strategy,
                        ?err,
                        "balance override strategy was not correct",
                    );
                }
            }
        }

        tracing::debug!(
            "none of the SLOAD slots appear to be the balance slot for token {:?}",
            token
        );

        Err(DetectionError::NotFound)
    }

    /// Verifies that a strategy controls the balance by writing the sentinel to
    /// its slot and reading `balanceOf` back, returning the confirmed strategy.
    ///
    /// If the sentinel reads back unshifted the strategy is correct as-is. If
    /// it reads back byte-shifted, the balance is packed above lower-order
    /// fields in the slot (e.g. AUSD packs `isFrozen` in the low byte, so
    /// the balance is the high 248 bits), and the detected shift is applied
    /// to the returned strategy from the same readback — no extra RPC. The
    /// `AaveV3AToken` variant is never packed and tolerates 1 wei of Aave
    /// ray-rounding difference.
    async fn verify_strategy(
        &self,
        token: Address,
        holder: Address,
        strategy: Strategy,
    ) -> Result<Strategy, DetectionError<TransportErrorKind>> {
        let test_balance = U256::from(SENTINEL);

        let overrides = strategy.state_override(&holder, &test_balance).await;
        if overrides.is_empty() {
            return Err(DetectionError::NotFound);
        }

        let balance = ERC20::Instance::new(token, self.web3.provider.clone())
            .balanceOf(holder)
            .state(overrides)
            .call()
            .await
            .map_err(|e| {
                DetectionError::Simulation(SimulationError::Other(anyhow::anyhow!(
                    "balanceOf call failed during strategy verification: {e}"
                )))
            })?;

        // The slot holds the balance unshifted.
        if verified_balance_matches(&strategy, balance, test_balance) {
            return Ok(strategy);
        }
        // Aave is never packed, so a mismatch there is a genuine miss.
        if matches!(strategy, Strategy::AaveV3AToken { .. }) {
            return Err(DetectionError::NotFound);
        }
        // Otherwise the balance may be packed above lower-order fields, so
        // `balanceOf` reads the sentinel back byte-shifted. Recover the shift
        // from the same readback and apply it.
        match detect_byte_shift(test_balance, balance) {
            Some(shift_bits) => Ok(strategy.with_shift(shift_bits)),
            None => Err(DetectionError::NotFound),
        }
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

/// Is the `balance` returned by `balanceOf` after applying the override
/// consistent with the `test_balance` we wrote? For `AaveV3AToken` we tolerate
/// 1 wei of difference because Aave's ray-div / ray-mul round-trip is not
/// identity by construction; every other strategy must match exactly.
fn verified_balance_matches(strategy: &Strategy, balance: U256, test_balance: U256) -> bool {
    match strategy {
        Strategy::AaveV3AToken { .. } => balance.abs_diff(test_balance) <= U256::ONE,
        _ => balance == test_balance,
    }
}

/// Left-shifts `amount` into a packed balance's bit position. Returns `None` if
/// the balance doesn't fit at this offset. Unlike `u64::checked_shl`, alloy's
/// `checked_shl` returns `None` exactly when the shifted-out bits are non-zero,
/// which is precisely the "doesn't fit above the lower-order fields" check.
fn packed_value(amount: U256, shift_bits: usize) -> Option<U256> {
    amount.checked_shl(shift_bits)
}

/// Returns the byte-aligned left-shift for which `probe >> shift == observed`,
/// i.e. the bit offset of a balance packed above lower-order fields in its slot
/// (e.g. AUSD's high-248-bit balance gives a shift of 8). `probe` must have
/// distinct bytes for the match to be unique.
fn detect_byte_shift(probe: U256, observed: U256) -> Option<usize> {
    // A zero readback means our write never reached `balanceOf` (wrong slot),
    // which is not a shift — and would otherwise match a shift that pushes the
    // whole probe out of the slot.
    if observed == U256::ZERO {
        return None;
    }
    (1..32usize)
        .map(|bytes| bytes * 8)
        .find(|&shift| probe >> shift == observed)
}

#[cfg(test)]
mod tests {
    use {super::*, alloy_primitives::address};

    #[test]
    fn packed_value_shifts_and_guards_overflow() {
        assert_eq!(
            packed_value(U256::from(1000u64), 0),
            Some(U256::from(1000u64))
        );
        assert_eq!(
            packed_value(U256::from(1000u64), 8),
            Some(U256::from(1000u64) << 8usize),
        );
        // A balance that no longer fits once shifted is rejected.
        assert_eq!(packed_value(U256::MAX, 8), None);
    }

    #[test]
    fn detect_byte_shift_recognizes_packed_balance() {
        let probe = U256::from(SENTINEL);
        // AUSD-style: isFrozen in the low byte, balance in the high bits.
        assert_eq!(detect_byte_shift(probe, probe >> 8usize), Some(8));
        assert_eq!(detect_byte_shift(probe, probe >> 24usize), Some(24));
        // Shift 0 is the plain DirectSlot case, not a packed one.
        assert_eq!(detect_byte_shift(probe, probe), None);
        // A zero readback is never a valid packed balance.
        assert_eq!(detect_byte_shift(probe, U256::ZERO), None);
        // Unrelated value is not a byte-shifted view of the probe.
        assert_eq!(detect_byte_shift(probe, U256::from(0xdeadu64)), None);
    }

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
                    shift_bits: 0,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot3,
                    shift_bits: 0,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot2,
                    shift_bits: 0,
                },
                Strategy::DirectSlot {
                    target_contract: contract2,
                    slot: slot3,
                    shift_bits: 0,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot1,
                    shift_bits: 0,
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
                    shift_bits: 0,
                },
                Strategy::SoladyMapping {
                    target_contract: contract,
                    shift_bits: 0,
                },
                Strategy::SolidityMapping {
                    target_contract: contract,
                    map_slot: U256::from(5),
                    shift_bits: 0,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot2,
                    shift_bits: 0,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot3,
                    shift_bits: 0,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot1,
                    shift_bits: 0,
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
                    shift_bits: 0,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: heuristic_slot,
                    shift_bits: 0,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot2,
                    shift_bits: 0,
                },
                Strategy::DirectSlot {
                    target_contract: contract,
                    slot: slot1,
                    shift_bits: 0,
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
                map_slot: U256::from(3),
                shift_bits: 0,
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
                shift_bits: 0,
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
                target_contract: address!("0000000000c5dc95539589fbd24be07c6c14eca4"),
                shift_bits: 0,
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
                map_slot: U256::from(51),
                shift_bits: 0,
            }
        );
    }

    // AUSD packs `{ bool isFrozen; uint248 balance }` into one slot, so the
    // balance is the high 248 bits and `balanceOf == slot >> 8`. Its balance
    // lives at an ERC-7201-namespaced base slot, so the detector resolves it to
    // a `DirectSlot` and verification recovers the 8-bit packing shift. Requires
    // an avalanche node (set the node URL `Web3::new_from_env` reads).
    #[ignore]
    #[tokio::test]
    async fn detects_packed_balance_slot_avalanche() {
        use crate::detector::DEFAULT_VERIFICATION_TIMEOUT;

        let ausd = address!("00000000efe302beaa2b3e6e1b18d08d69a9012a");
        let detector = Detector::new(Web3::new_from_env(), 60, DEFAULT_VERIFICATION_TIMEOUT, 100);

        let strategy = detector
            .detect(ausd, Address::with_last_byte(1))
            .await
            .unwrap();

        match strategy {
            Strategy::DirectSlot {
                target_contract,
                shift_bits,
                ..
            } => {
                assert_eq!(target_contract, ausd);
                assert_eq!(shift_bits, 8);
            }
            other => panic!("expected DirectSlot with shift, got {other:?}"),
        }
    }
}
