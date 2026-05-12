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
    std::{iter, sync::Mutex, time::Duration},
};

/// These are the solady magic bytes for user allowances
/// <https://github.com/Vectorized/solady/blob/main/src/tokens/ERC20.sol#L90>
const ALLOWANCE_SLOT_SEED: &[u8] = &[0x7f, 0x5e, 0x9f, 0x20];

/// Produces approval strategy candidates from a list of SLOAD-accessed slots.
/// This function assumes SLOAD-accessed slots are returned in order that they
/// were accessed in the relevant call (`balanceOf()`, or `allowance()`).
/// Since it's most likely that the last slots are the important ones this
/// function returns strategies for the slots in **reverse** order.
///
/// Tries Solady and both Solidity double-mapping orderings (owner→spender and
/// spender→owner, for each base map_slot index 0..depth). If `depth == 0`
/// **no** such strategies will be tried and returned.
/// In that case this function can only return `DirectSlot` or `SoladyMapping`
/// strategies.
pub(crate) fn find_plausible_strategies_for_slots(
    storage_slots: &[(Address, B256)],
    owner: &Address,
    spender: &Address,
    heuristic_depth: usize,
) -> Vec<ApprovalStrategy> {
    let mut heuristic_candidates = Vec::new();
    let mut fallback_candidates = Vec::new();

    for (contract, observed_slot) in storage_slots.iter().rev() {
        let solady = ApprovalStrategy::SoladyMapping {
            target_contract: *contract,
        };
        if solady.slot(*owner, *spender).1 == *observed_slot {
            heuristic_candidates.push(solady);
            continue;
        }

        let mut matched = false;
        for i in 0..heuristic_depth {
            let map_slot = U256::from(i);

            let owner_to_spender = ApprovalStrategy::SolidityMappingOwnerToSpender {
                target_contract: *contract,
                map_slot,
            };
            if owner_to_spender.slot(*owner, *spender).1 == *observed_slot {
                heuristic_candidates.push(owner_to_spender);
                matched = true;
                break;
            }

            let spender_to_owner = ApprovalStrategy::SolidityMappingSpenderToOwner {
                target_contract: *contract,
                map_slot,
            };
            if spender_to_owner.slot(*owner, *spender).1 == *observed_slot {
                heuristic_candidates.push(spender_to_owner);
                matched = true;
                break;
            }
        }

        if !matched {
            fallback_candidates.push(ApprovalStrategy::DirectSlot {
                target_contract: *contract,
                slot: *observed_slot,
            });
        }
    }

    heuristic_candidates.extend(fallback_candidates);
    heuristic_candidates
}

/// Parameters for computing an approval state override.
#[derive(Clone, Debug)]
pub struct ApprovalOverrideRequest {
    pub token: Address,
    pub owner: Address,
    pub spender: Address,
    pub amount: U256,
}

/// Detected allowance storage strategy for a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalStrategy {
    /// Standard Solidity double mapping with owner as the outer key:
    /// `mapping(address owner => mapping(address spender => uint256))` at
    /// `map_slot`.
    SolidityMappingOwnerToSpender {
        target_contract: Address,
        map_slot: U256,
    },
    /// Solidity double mapping with spender as the outer key:
    /// `mapping(address spender => mapping(address owner => uint256))` at
    /// `map_slot`.
    SolidityMappingSpenderToOwner {
        target_contract: Address,
        map_slot: U256,
    },
    /// Solady ERC-20 packed allowance layout.
    SoladyMapping { target_contract: Address },
    /// Fallback for tokens with non-standard allowance storage: the exact slot
    /// observed during tracing is stored and used verbatim. Because the slot
    /// was observed for a specific `(owner, spender)` pair and may encode them
    /// in an unknown way, this strategy is treated as pair-specific.
    DirectSlot {
        target_contract: Address,
        slot: B256,
    },
}

impl ApprovalStrategy {
    /// Computes the `(contract, storage_slot)` pair for a specific
    /// `(owner, spender)` pair.
    pub(crate) fn slot(&self, owner: Address, spender: Address) -> (Address, B256) {
        match self {
            Self::SolidityMappingOwnerToSpender {
                target_contract,
                map_slot,
            } => {
                let inner = mapping_slot_hash(&owner, &map_slot.to_be_bytes::<32>());
                let slot = mapping_slot_hash(&spender, &inner.0);
                (*target_contract, slot)
            }
            Self::SolidityMappingSpenderToOwner {
                target_contract,
                map_slot,
            } => {
                let inner = mapping_slot_hash(&spender, &map_slot.to_be_bytes::<32>());
                let slot = mapping_slot_hash(&owner, &inner.0);
                (*target_contract, slot)
            }
            Self::SoladyMapping { target_contract } => {
                // keccak256(owner_20 ‖ 0x00×8 ‖ 0x7f5e9f20 ‖ spender_20)  [52 bytes]
                let mut buf = [0u8; 52];
                buf[0..20].copy_from_slice(owner.as_slice());
                buf[28..32].copy_from_slice(ALLOWANCE_SLOT_SEED);
                buf[32..52].copy_from_slice(spender.as_slice());
                (*target_contract, keccak256(buf))
            }
            Self::DirectSlot {
                target_contract,
                slot,
            } => (*target_contract, *slot),
        }
    }

    pub(crate) fn is_valid_for_all_pairs(&self) -> bool {
        matches!(
            self,
            Self::SolidityMappingOwnerToSpender { .. }
                | Self::SolidityMappingSpenderToOwner { .. }
                | Self::SoladyMapping { .. }
        )
    }

    /// Builds the state override for `(owner, spender, amount)`.
    pub(crate) fn state_override(
        &self,
        owner: Address,
        spender: Address,
        amount: U256,
    ) -> AddressMap<AccountOverride> {
        let (target_contract, slot) = self.slot(owner, spender);
        let account_override = AccountOverride::default()
            .with_state_diff(iter::once((slot, B256::new(amount.to_be_bytes::<32>()))));
        iter::once((target_contract, account_override)).collect()
    }
}

type Cache = SizedCache<(Address, Option<(Address, Address)>), Option<ApprovalStrategy>>;

/// Heuristic approval override detector with integrated caching.
///
/// Owns the Web3 handle, detection parameters, and the per-token strategy
/// cache. Strategies that can compute the correct slot for any `(owner,
/// spender)` pair (Solidity mappings, Solady) are cached under `(token, None)`.
/// `DirectSlot` strategies, which encode a specific observed slot, are cached
/// under `(token, Some((owner, spender)))`.
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

    /// Returns the cached detection result for `(token, owner, spender)`,
    /// running detection if not yet cached.
    ///
    /// Pair-agnostic strategies (Solidity mappings, Solady) are cached under
    /// `(token, None)` and returned for any pair. `DirectSlot` strategies are
    /// cached under `(token, Some((owner, spender)))` because the observed slot
    /// may encode the specific pair.
    pub(crate) async fn detect(
        &self,
        token: Address,
        owner: Address,
        spender: Address,
    ) -> Option<ApprovalStrategy> {
        tracing::trace!(?token, "attempting to auto-detect approval slot");

        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(strategy) = cache.cache_get(&(token, None)) {
                tracing::trace!(?token, "cache hit (strategy valid for all pairs)");
                return strategy.clone();
            }
            if let Some(strategy) = cache.cache_get(&(token, Some((owner, spender)))) {
                tracing::trace!(
                    ?token,
                    ?owner,
                    ?spender,
                    "cache hit (pair-specific strategy)"
                );
                return strategy.clone();
            }
        }

        let result = self.detect_uncached(token, owner, spender).await;

        if matches!(&result, Ok(_) | Err(DetectionError::NotFound)) {
            tracing::debug!(?token, ?result, "caching auto-detected approval strategy");
            if let Ok(strategy) = result.as_ref() {
                let cache_key = (
                    token,
                    (!strategy.is_valid_for_all_pairs()).then_some((owner, spender)),
                );
                self.cache
                    .lock()
                    .unwrap()
                    .cache_set(cache_key, Some(strategy.clone()));
            } else {
                self.cache
                    .lock()
                    .unwrap()
                    .cache_set((token, Some((owner, spender))), None);
            }
        } else {
            tracing::warn!(?token, ?result, "error auto-detecting approval strategy");
        }

        result.ok()
    }

    async fn detect_uncached(
        &self,
        token: Address,
        owner: Address,
        spender: Address,
    ) -> Result<ApprovalStrategy, DetectionError<TransportErrorKind>> {
        let allowance_call = ERC20::ERC20::allowanceCall { owner, spender };
        let calldata = allowance_call.abi_encode();

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
            tracing::debug!(?token, "no SLOAD operations in allowance trace");
            return Err(DetectionError::NotFound);
        }

        let strategies = find_plausible_strategies_for_slots(
            &storage_slots,
            &owner,
            &spender,
            self.probing_depth.into(),
        );

        for (i, strategy) in strategies
            .iter()
            .take(self.probing_depth.into())
            .enumerate()
        {
            let result = tokio::time::timeout(
                self.verification_timeout,
                self.verify_approval_strategy(token, owner, spender, strategy),
            )
            .await;

            match result {
                Ok(Ok(())) => {
                    tracing::debug!(
                        ?token,
                        ?owner,
                        ?spender,
                        ?strategy,
                        iterations = i + 1,
                        "verified approval strategy",
                    );
                    return Ok(strategy.clone());
                }
                Err(_) => {
                    tracing::warn!(
                        ?token,
                        ?strategy,
                        "approval strategy verification timed out, skipping",
                    );
                }
                Ok(Err(_)) => {}
            }
        }

        tracing::debug!(?token, "no approval slot found for token");
        Err(DetectionError::NotFound)
    }

    async fn verify_approval_strategy(
        &self,
        token: Address,
        owner: Address,
        spender: Address,
        strategy: &ApprovalStrategy,
    ) -> Result<(), DetectionError<TransportErrorKind>> {
        let test_amount = U256::from(0x1337_1337_1337_1337_u64);

        let (target_contract, slot) = strategy.slot(owner, spender);
        let state_override = AccountOverride {
            state_diff: Some(
                iter::once((
                    slot,
                    alloy_primitives::B256::new(test_amount.to_be_bytes::<32>()),
                ))
                .collect(),
            ),
            ..Default::default()
        };
        let overrides: alloy_primitives::map::AddressMap<AccountOverride> =
            iter::once((target_contract, state_override)).collect();

        let token_contract = ERC20::Instance::new(token, self.web3.provider.clone());
        let allowance = token_contract
            .allowance(owner, spender)
            .state(overrides)
            .call()
            .await
            .map_err(|e| {
                DetectionError::Simulation(SimulationError::Other(anyhow::anyhow!(
                    "allowance call failed during strategy verification: {e}"
                )))
            })?;

        (allowance == test_amount)
            .then_some(())
            .ok_or(DetectionError::NotFound)
    }
}

impl std::fmt::Debug for Detector {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("approval::Detector")
            .field("probing_depth", &self.probing_depth)
            .field("verification_timeout", &self.verification_timeout)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{StateOverrides, StateOverriding},
        alloy_primitives::address,
        alloy_rpc_types::state::StateOverride,
        contracts::ERC20::ERC20,
        ethrpc::Web3,
    };

    #[test]
    fn solidity_mapping_owner_to_spender_slot() {
        let token = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let owner = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let spender = Address::with_last_byte(1);

        let strategy = ApprovalStrategy::SolidityMappingOwnerToSpender {
            target_contract: token,
            map_slot: U256::ONE,
        };

        let (contract, slot) = strategy.slot(owner, spender);
        assert_eq!(contract, token);

        let inner = mapping_slot_hash(&owner, &U256::from(1u64).to_be_bytes::<32>());
        let expected = mapping_slot_hash(&spender, &inner.0);
        assert_eq!(slot, expected);
    }

    #[test]
    fn solidity_mapping_spender_to_owner_slot() {
        let token = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let owner = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let spender = Address::with_last_byte(1);

        let strategy = ApprovalStrategy::SolidityMappingSpenderToOwner {
            target_contract: token,
            map_slot: U256::from(1),
        };

        let (contract, slot) = strategy.slot(owner, spender);
        assert_eq!(contract, token);

        let inner = mapping_slot_hash(&spender, &U256::from(1u64).to_be_bytes::<32>());
        let expected = mapping_slot_hash(&owner, &inner.0);
        assert_eq!(slot, expected);
    }

    #[test]
    fn solady_approval_slot() {
        let token = address!("0000000000c5dc95539589fbd24be07c6c14eca4");
        let owner = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let spender = Address::with_last_byte(1);

        let strategy = ApprovalStrategy::SoladyMapping {
            target_contract: token,
        };

        let (contract, slot) = strategy.slot(owner, spender);
        assert_eq!(contract, token);

        // Compute expected: keccak256(owner_20 ‖ 0x00×8 ‖ 0x7f5e9f20 ‖ spender_20)
        let mut buf = [0u8; 52];
        buf[0..20].copy_from_slice(owner.as_slice());
        buf[28..32].copy_from_slice(&[0x7f, 0x5e, 0x9f, 0x20]);
        buf[32..52].copy_from_slice(spender.as_slice());
        assert_eq!(slot, keccak256(buf));
    }

    #[ignore]
    #[tokio::test]
    async fn override_allowance_simple() {
        let token = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let owner = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let spender = Address::with_last_byte(1);
        let amount = U256::from(123123123);

        let web3 = Web3::new_from_env();
        let helper = StateOverrides::new(web3.clone());

        let state_overrides: StateOverride = helper
            .approval_override(ApprovalOverrideRequest {
                owner,
                spender,
                token,
                amount,
            })
            .await
            .into_iter()
            .collect();

        let erc20 = ERC20::new(token, &web3.provider);
        let allowance_with_override = erc20
            .allowance(owner, spender)
            .state(state_overrides)
            .call()
            .await
            .unwrap();

        assert_eq!(allowance_with_override, amount);
    }

    #[ignore]
    #[tokio::test]
    async fn override_allowance_solady() {
        let token = address!("0000000000c5dc95539589fbd24be07c6c14eca4");
        let owner = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let spender = Address::with_last_byte(1);
        let amount = U256::from(123123123);

        let web3 = Web3::new_from_env();
        let helper = StateOverrides::new(web3.clone());

        let state_overrides: StateOverride = helper
            .approval_override(ApprovalOverrideRequest {
                owner,
                spender,
                token,
                amount,
            })
            .await
            .into_iter()
            .collect();

        let erc20 = ERC20::new(token, &web3.provider);
        let allowance_with_override = erc20
            .allowance(owner, spender)
            .state(state_overrides)
            .call()
            .await
            .unwrap();

        assert_eq!(allowance_with_override, amount);
    }

    #[ignore]
    #[tokio::test]
    async fn override_balance_solady() {
        let token = address!("0000000000c5dc95539589fbd24be07c6c14eca4");
        let holder = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let amount = U256::from(123123123);

        let web3 = Web3::new_from_env();
        let helper = StateOverrides::new(web3.clone());

        let state_overrides: StateOverride = helper
            .balance_override(crate::BalanceOverrideRequest {
                token,
                holder,
                amount,
            })
            .await
            .into_iter()
            .collect();

        let erc20 = ERC20::new(token, &web3.provider);
        let allowance_with_override = erc20
            .balanceOf(holder)
            .state(state_overrides)
            .call()
            .await
            .unwrap();

        assert_eq!(allowance_with_override, amount);
    }

    #[test]
    fn test_unmatched_slots_become_direct_slot() {
        let contract = Address::with_last_byte(2);
        let owner = Address::repeat_byte(1);
        let spender = Address::repeat_byte(2);

        let slot1 = B256::with_last_byte(0xe1);
        let slot2 = B256::with_last_byte(0xe2);
        let slots = vec![(contract, slot1), (contract, slot2)];

        let strategies = find_plausible_strategies_for_slots(&slots, &owner, &spender, 10);

        assert_eq!(
            strategies,
            vec![
                ApprovalStrategy::DirectSlot {
                    target_contract: contract,
                    slot: slot2,
                },
                ApprovalStrategy::DirectSlot {
                    target_contract: contract,
                    slot: slot1,
                },
            ]
        );
    }

    #[test]
    fn test_heuristic_before_direct_slot_fallback() {
        let contract = Address::with_last_byte(2);
        let owner = Address::repeat_byte(1);
        let spender = Address::repeat_byte(2);

        // Compute a real Solidity OwnerToSpender slot with map_slot = 1
        let heuristic_strategy = ApprovalStrategy::SolidityMappingOwnerToSpender {
            target_contract: contract,
            map_slot: U256::from(1),
        };
        let heuristic_slot = heuristic_strategy.slot(owner, spender).1;
        let fallback_slot = B256::with_last_byte(0xff);

        let slots = vec![(contract, fallback_slot), (contract, heuristic_slot)];

        let strategies = find_plausible_strategies_for_slots(&slots, &owner, &spender, 10);

        assert_eq!(
            strategies,
            vec![
                ApprovalStrategy::SolidityMappingOwnerToSpender {
                    target_contract: contract,
                    map_slot: U256::from(1),
                },
                ApprovalStrategy::DirectSlot {
                    target_contract: contract,
                    slot: fallback_slot,
                },
            ]
        );
    }

    #[test]
    fn test_zero_heuristic_depth_all_direct_slot() {
        let contract = Address::with_last_byte(2);
        let owner = Address::repeat_byte(1);
        let spender = Address::repeat_byte(2);

        // Compute a real Solidity slot — with depth 0 it must NOT match
        let heuristic_strategy = ApprovalStrategy::SolidityMappingOwnerToSpender {
            target_contract: contract,
            map_slot: U256::from(1),
        };
        let heuristic_slot = heuristic_strategy.slot(owner, spender).1;
        let other_slot = B256::with_last_byte(0x42);

        let slots = vec![(contract, other_slot), (contract, heuristic_slot)];

        let strategies = find_plausible_strategies_for_slots(&slots, &owner, &spender, 0);

        assert_eq!(
            strategies,
            vec![
                ApprovalStrategy::DirectSlot {
                    target_contract: contract,
                    slot: heuristic_slot,
                },
                ApprovalStrategy::DirectSlot {
                    target_contract: contract,
                    slot: other_slot,
                },
            ]
        );
    }
}
