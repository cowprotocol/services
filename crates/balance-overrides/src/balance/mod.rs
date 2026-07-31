pub(crate) mod aave;

use {
    crate::detector::{DetectionError, SimulationError, extract_sload_slots, mapping_slot_hash},
    alloy_eips::BlockId,
    alloy_primitives::{Address, B256, TxKind, U256, address, keccak256, map::AddressMap},
    alloy_provider::ext::DebugApi,
    alloy_rpc_types::{
        TransactionInput,
        TransactionRequest,
        state::AccountOverride,
        trace::geth::GethDebugTracingCallOptions,
    },
    alloy_sol_types::SolCall,
    alloy_transport::TransportErrorKind,
    contracts::ERC20,
    ethrpc::AlloyProvider,
    moka::sync::Cache,
    std::{collections::HashMap, iter, time::Duration},
};

/// These are the solady magic bytes for user balances
/// https://github.com/Vectorized/solady/blob/main/src/tokens/ERC20.sol#L81
const BALANCE_SLOT_SEED: &[u8] = &[0x87, 0xa2, 0x11, 0xa2];

/// Distinct-byte sentinel written to a candidate balance slot during
/// verification. Every byte is distinct and non-zero, so the value `balanceOf`
/// reads back is a unique contiguous byte-slice of it (see `detect_shift`). Its
/// position gives the packing shift for a balance stored above lower-order
/// fields, e.g. AUSD packs `isFrozen` in the low byte, so the balance is the
/// high 248 bits and `shift_bits == 8`. A readback that is not a slice means
/// the slot is not the balance. The full 32 bytes cover any byte-aligned shift,
/// including narrow fields: a `uint96` balance (UNI, COMP) reads back as the
/// low-byte suffix and resolves to `shift_bits == 0`.
const SENTINEL: [u8; 32] =
    alloy_primitives::hex!("0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20");

/// Marker address for native ETH - the non-ERC20 gas token on many chains.
const NATIVE_ETH: Address = address!("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");

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
/// The `AaveV3AToken` variant owns a cloned provider handle, so it computes its
/// override without an external provider reference threaded through.
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
        provider: AlloyProvider,
    },
    NativeEth,
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
                provider,
            } => {
                return match aave::build_override(
                    provider,
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
            Self::NativeEth => {
                return iter::once((*holder, AccountOverride::default().with_balance(*amount)))
                    .collect();
            }
        };

        // Pack the balance into its slot position. `checked_shl` returns `None`
        // only when the balance is too large to fit above the lower-order
        // fields, which a real balance never is, so log it if it ever happens.
        let Some(value) = amount.checked_shl(shift_bits) else {
            tracing::warn!(
                ?target_contract,
                ?holder,
                %amount,
                shift_bits,
                "balance does not fit packed slot, skipping override",
            );
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

    /// Sets the packing shift on a balance strategy. No-op for `AaveV3AToken`
    /// and `NativeEth`, which are never packed.
    fn with_shift(mut self, shift_bits: usize) -> Self {
        match &mut self {
            Self::SolidityMapping { shift_bits: s, .. }
            | Self::SoladyMapping { shift_bits: s, .. }
            | Self::DirectSlot { shift_bits: s, .. } => *s = shift_bits,
            Self::AaveV3AToken { .. } | Self::NativeEth => {}
        }
        self
    }
}

/// Compare by addresses only; `provider` is intentionally excluded since two
/// strategies for the same token/pool/underlying are semantically equivalent
/// regardless of which provider handle they carry.
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
            (Self::NativeEth, Self::NativeEth) => true,
            _ => false,
        }
    }
}

impl Eq for Strategy {}

/// Heuristic balance override detector with integrated caching.
///
/// Owns the provider handle, detection parameters, and the per-token strategy
/// cache. `AaveV3AToken` strategies in the cache carry a cloned provider handle
/// so they can compute overrides without any external dependency.
pub(crate) struct Detector {
    provider: AlloyProvider,
    probing_depth: u8,
    verification_timeout: Duration,
    pub(crate) cache: Cache<(Address, Option<Address>), Option<Strategy>>,
}

impl Detector {
    pub fn new(
        provider: AlloyProvider,
        probing_depth: u8,
        verification_timeout: Duration,
        cache_size: u64,
    ) -> Self {
        Self {
            provider,
            probing_depth,
            verification_timeout,
            cache: Cache::builder().max_capacity(cache_size).build(),
        }
    }

    /// Returns the cached detection result for `(token, holder)`, running
    /// detection if not yet cached.
    pub async fn detect(&self, token: Address, holder: Address) -> Option<Strategy> {
        tracing::trace!(?token, "attempting to auto-detect balance slot");

        {
            if let Some(strategy_opt) = self.cache.get(&(token, None)) {
                tracing::trace!(?token, "cache hit (strategy valid for all holders)");
                return strategy_opt;
            }
            if let Some(strategy_opt) = self.cache.get(&(token, Some(holder))) {
                tracing::trace!(?token, ?holder, "cache hit (holder-specific strategy)");
                return strategy_opt;
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
                self.cache.insert(cache_key, Some(strategy.clone()));
            }
            Err(DetectionError::NotFound) => {
                tracing::debug!(?token, "caching token as unsupported");
                self.cache.insert((token, Some(holder)), None);
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
        if token == NATIVE_ETH {
            return Ok(Strategy::NativeEth);
        }
        // Aave fast-path. If the token self-identifies as a v3 aToken and
        // the pool confirms it, try the canonical `_userState` slot
        // directly — no `debug_traceCall` needed. An Aave v3 fork that
        // moved `_userState` to a different slot won't verify here and
        // will fall through to the generic trace-based path, which only
        // ever returns non-Aave strategies; such a fork needs an explicit
        // hardcoded config entry.
        if let Some((pool, underlying)) = aave::probe_aave_token(&self.provider, token).await {
            let candidate = Strategy::AaveV3AToken {
                target_contract: token,
                pool,
                underlying,
                provider: self.provider.clone(),
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

    /// Verifies that a strategy controls the balance by writing a sentinel to
    /// its slot and reading `balanceOf` back, returning the confirmed strategy.
    ///
    /// Generic strategies use the full [`SENTINEL`]: `detect_shift` finds the
    /// readback as a byte-slice of it, which verifies the slot and recovers the
    /// packing shift (0 when unpacked) from the same readback, no extra RPC.
    /// The `AaveV3AToken` variant never packs and stores its scaled balance
    /// in a `uint128`, so it uses the small [`aave::SENTINEL`] and accepts a
    /// round-trip within 1 wei of Aave's ray rounding.
    async fn verify_strategy(
        &self,
        token: Address,
        holder: Address,
        strategy: Strategy,
    ) -> Result<Strategy, DetectionError<TransportErrorKind>> {
        let is_aave = matches!(strategy, Strategy::AaveV3AToken { .. });
        let sentinel = if is_aave {
            U256::from_be_bytes(aave::SENTINEL)
        } else {
            U256::from_be_bytes(SENTINEL)
        };

        let overrides = strategy.state_override(&holder, &sentinel).await;
        if overrides.is_empty() {
            return Err(DetectionError::NotFound);
        }

        let balance = ERC20::Instance::new(token, self.provider.clone())
            .balanceOf(holder)
            .state(overrides)
            .call()
            .await
            .map_err(|e| {
                DetectionError::Simulation(SimulationError::Other(anyhow::anyhow!(
                    "balanceOf call failed during strategy verification: {e}"
                )))
            })?;

        if is_aave {
            // Aave's ray-div / ray-mul round-trip is not identity, so allow 1
            // wei of difference.
            return (balance.abs_diff(sentinel) <= U256::ONE)
                .then_some(strategy)
                .ok_or(DetectionError::NotFound);
        }

        // Locate the readback as a contiguous byte-slice of the sentinel. Its
        // position gives the packing shift, 0 when unpacked. A non-match means
        // this slot is not the balance.
        match detect_shift(balance) {
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

/// Recovers the packing shift by locating the `balanceOf` readback as a
/// contiguous byte-slice of [`SENTINEL`]. The sentinel's bytes are distinct and
/// non-zero, so the slice is unique: the number of bytes sitting below it is
/// the shift, and a readback that is not a slice means the slot is not the
/// balance. Returns the left-shift in bits (0 for an unpacked balance).
fn detect_shift(observed: U256) -> Option<usize> {
    // Strip leading zero bytes to get the field's bytes. The field is a slice of
    // the all-non-zero sentinel, so its top byte is non-zero and trimming never
    // eats into it. An all-zero readback has no non-zero byte (the write never
    // reached `balanceOf`), so `position` returns None and we bail.
    let bytes = observed.to_be_bytes::<32>();
    let needle = &bytes[bytes.iter().position(|&b| b != 0)?..];
    let start = SENTINEL.windows(needle.len()).position(|w| w == needle)?;
    Some(SENTINEL.len().checked_sub(start + needle.len())? * 8)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::detector::DEFAULT_VERIFICATION_TIMEOUT,
        alloy_primitives::address,
        contracts::WETH9,
        ethrpc::Web3,
    };

    #[test]
    fn detect_shift_locates_packed_balance() {
        let sentinel = U256::from_be_bytes(SENTINEL);
        // Unpacked: the full sentinel reads back as-is.
        assert_eq!(detect_shift(sentinel), Some(0));
        // AUSD-style: isFrozen in the low byte, balance in the high 248 bits, so
        // `balanceOf == slot >> 8`.
        assert_eq!(detect_shift(sentinel >> 8usize), Some(8));
        assert_eq!(detect_shift(sentinel >> 24usize), Some(24));
        // Narrow uint96 field (UNI/COMP): the low 12 bytes, the sentinel's
        // suffix, still resolve to shift 0.
        let mask96 = (U256::from(1u64) << 96) - U256::from(1u64);
        assert_eq!(detect_shift(sentinel & mask96), Some(0));
        // Middle field: a 96-bit balance at bit offset 8 (lower-order fields
        // below, more above). balanceOf reads it shifted down to the low bits,
        // so the readback is `(slot >> 8) & mask`, recovered as shift 8.
        assert_eq!(detect_shift((sentinel >> 8usize) & mask96), Some(8));
        // A zero readback means the write never reached `balanceOf`.
        assert_eq!(detect_shift(U256::ZERO), None);
        // Unrelated value is not a byte-slice of the sentinel.
        assert_eq!(detect_shift(U256::from(0xdeadu64)), None);
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
        let detector = Detector::new(
            Web3::new_from_env().provider,
            60,
            DEFAULT_VERIFICATION_TIMEOUT,
            100,
        );

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
        let detector = Detector::new(
            Web3::new_from_env().provider,
            60,
            DEFAULT_VERIFICATION_TIMEOUT,
            100,
        );

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
        let ausd = address!("00000000efe302beaa2b3e6e1b18d08d69a9012a");
        let detector = Detector::new(
            Web3::new_from_env().provider,
            60,
            DEFAULT_VERIFICATION_TIMEOUT,
            100,
        );

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

    #[ignore]
    #[tokio::test]
    async fn detects_native_eth() {
        let web3 = Web3::new_from_env();
        let weth = WETH9::Instance::deployed(&web3.provider).await.unwrap();
        let detector = Detector::new(web3.provider.clone(), 60, DEFAULT_VERIFICATION_TIMEOUT, 100);

        let user = Address::random();
        let amount = U256::MAX / U256::from(2);

        let strategy = detector.detect(NATIVE_ETH, user).await.unwrap();

        std::assert_matches!(strategy, Strategy::NativeEth);

        // ETH is not an ERC20 token so we can't do an `eth_call` on `balanceOf()`.
        // Additionally `eth_getBalance` does not support state overrides.
        // So to infer that our override works we assert that we can wrap the
        // desired amount of ETH to WETH.
        let state = strategy.state_override(&user, &amount).await;
        assert!(
            weth.deposit()
                .value(amount)
                .from(user)
                .state(state)
                .call()
                .await
                .is_ok()
        );

        assert!(
            weth.deposit()
                .value(amount)
                .from(user)
                .call()
                .await
                .is_err()
        );
    }
}
