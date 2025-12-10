use {
    super::Strategy,
    crate::tenderly_api::SimulationError,
    alloy::{
        eips::BlockId,
        primitives::{Address, B256, TxKind},
        providers::ext::DebugApi,
        rpc::types::{
            TransactionInput,
            TransactionRequest,
            trace::geth::{GethDebugTracingCallOptions, GethTrace},
        },
        sol_types::SolCall,
    },
    anyhow::Context,
    contracts::alloy::ERC20,
    ethcontract::{H160, H256, U256, state_overrides::StateOverride},
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    maplit::hashmap,
    std::{
        collections::{HashMap, HashSet},
        fmt::{self, Debug, Formatter},
        sync::Arc,
    },
    thiserror::Error,
    web3::signing,
};

/// A heuristic balance override detector based on `eth_call` simulations.
///
/// This has the exact same node requirements as trade verification.
#[derive(Clone)]
pub struct Detector(Arc<Inner>);

pub struct Inner {
    probing_depth: u8,
    web3: ethrpc::Web3,
}

impl std::ops::Deref for Detector {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Helper function to put together separate StateOverride objects in a address
/// hash map. Useful for condensing multiple state overrides into a single call
/// request. This is specifically designed to work with the state overrides that
/// come from the Detector, and this function does not have any logic to merge
/// together any fields other than `state_diff`.
fn merge_state_diffs_in_overrides(
    overrides: Vec<HashMap<H160, StateOverride>>,
) -> HashMap<H160, StateOverride> {
    let mut merged = HashMap::new();

    for override_map in overrides {
        for (address, state_override) in override_map {
            merged
                .entry(address)
                .and_modify(|existing: &mut StateOverride| {
                    if let (Some(existing_diff), Some(new_diff)) =
                        (&mut existing.state_diff, &state_override.state_diff)
                    {
                        existing_diff.extend(new_diff.clone());
                    }
                })
                .or_insert(state_override);
        }
    }

    merged
}

/// Used by detect_with_trace when there are multiple to increase the chances we
/// find the correct storage slot quickly. Strategies employed:
/// 1. Reverse because the most recently accessed storage slots are most likely
///    tobe the return value of `balanceOf`
/// 2. Use the Solidity mapping hashes as a heuristic and prioritize scanning
///    those storage slots first
fn sort_storage_overrides(
    storage_slots: &mut [(Address, B256)],
    holder: &Address,
    heuristic_depth: usize,
) {
    // We can also use heuristics to sort by slots most likely to actually be the
    // balance slot
    let mut heuristic_slots = HashSet::new();

    for i in 0..heuristic_depth {
        let mut buf = [0; 64];
        buf[12..32].copy_from_slice(holder.as_slice());
        U256::from(i).to_big_endian(&mut buf[32..64]);
        heuristic_slots.insert(B256::from(signing::keccak256(&buf)));
    }

    // sort by whether or not its a heuristic slot or not (stable so it wont change
    // the relative order of equal elements)
    storage_slots.sort_by_key(|v| heuristic_slots.contains(&v.1));

    // Iterate through slots in reverse order (last accessed is most likely the
    // balance)
    storage_slots.reverse();
}

impl Detector {
    /// Creates a new balance override detector.
    pub fn new(web3: ethrpc::Web3, probing_depth: u8) -> Self {
        Self(Arc::new(Inner {
            web3,
            probing_depth,
        }))
    }

    fn generate_strategies(&self, target_contract: H160) -> Vec<StrategyHelper> {
        // First test storage slots that don't need guesswork.
        let mut strategies = vec![
            Strategy::SoladyMapping { target_contract },
            Strategy::SolidityMapping {
                target_contract,
                map_slot: U256::from(OPEN_ZEPPELIN_ERC20_UPGRADEABLE),
            },
        ];

        // For each entry point probe the first n following slots.
        let entry_points = [
            // solc lays out memory linearly starting at 0 by default
            "0000000000000000000000000000000000000000000000000000000000000000",
        ];
        for start_slot in entry_points {
            let mut map_slot = U256::from(start_slot);
            for _ in 0..self.probing_depth {
                strategies.push(Strategy::SolidityMapping {
                    target_contract,
                    map_slot,
                });
                map_slot += U256::one();
            }
        }

        strategies
            .into_iter()
            .enumerate()
            .map(|(index, strategy)| StrategyHelper::new(strategy, index))
            .collect()
    }

    /// Tries to detect the balance override strategy for the specified token.
    /// Returns an `Err` if it cannot detect the strategy or an internal
    /// simulation fails.
    pub async fn detect(
        &self,
        token: Address,
        holder: Address,
    ) -> Result<Strategy, DetectionError> {
        // First try the trace-based detection if the node supports it
        let trace_strategy = self.detect_with_trace(token, holder).await;
        if let Ok(strategy) = trace_strategy {
            tracing::debug!(?token, ?strategy, "Trace-based detection succeeded");
            return Ok(strategy);
        } else {
            tracing::debug!(
                ?token,
                ?trace_strategy,
                "Trace-based detection failed, falling back to heuristic detection",
            );
        }

        // Fall back to the original heuristic-based detection
        let strategies = self.generate_strategies(token.into_legacy());
        let token_contract = ERC20::Instance::new(token, self.web3.alloy.clone());
        let overrides = merge_state_diffs_in_overrides(
            strategies
                .iter()
                .map(|helper| {
                    helper
                        .strategy
                        .state_override(&holder.into_legacy(), &helper.balance)
                })
                .collect(),
        );

        let balance = token_contract
            .balanceOf(holder)
            .state(overrides.into_alloy())
            .call()
            .await
            .context("eth_call with state overrides failed")
            .map_err(|e| DetectionError::Simulation(SimulationError::Other(e)))?;

        strategies
            .iter()
            .find_map(|helper| {
                (helper.balance.into_alloy() == balance).then_some(helper.strategy.clone())
            })
            .ok_or(DetectionError::NotFound)
    }

    /// Detects the balance storage slot using debug_traceCall, similar to
    /// Foundry's `deal`. This traces a balanceOf call and finds which
    /// storage slot is accessed. If more than one slot is accessed, we reverse
    /// by order accessed, sort by if the slot was one that would normally
    /// be seen in  solidity mapping, and test one by one to see if the
    /// override is effective.
    async fn detect_with_trace(
        &self,
        token: Address,
        holder: Address,
    ) -> Result<Strategy, DetectionError> {
        let balance_of_call = ERC20::ERC20::balanceOfCall { account: holder };
        let calldata = balance_of_call.abi_encode();

        let call_request = TransactionRequest {
            to: Some(TxKind::Call(token)),
            input: TransactionInput::new(calldata.into()),
            ..Default::default()
        };

        let trace = self
            .web3
            .alloy
            .debug_trace_call(
                call_request,
                BlockId::latest(),
                GethDebugTracingCallOptions::default(),
            )
            .await
            .map_err(|e| {
                tracing::debug!(?token, error = ?e, "debug_traceCall not supported for token");
                DetectionError::Simulation(SimulationError::Other(anyhow::anyhow!(
                    "debug_traceCall failed: {e}"
                )))
            })?;

        // Extract storage slots accessed via SLOAD operations
        let mut storage_slots = self.extract_sload_slots(trace, token);

        if storage_slots.is_empty() {
            tracing::debug!("no SLOAD operations found in trace for token {:?}", token);
            return Err(DetectionError::NotFound);
        }

        if storage_slots.len() == 1 {
            let slot = storage_slots[0];
            tracing::debug!(
                storage_context = ?slot.0,
                slot = ?slot.1,
                ?slot,
                iterations = 0,
                "detected balance slot via trace (single SLOAD) for token",
            );
            return Ok(Strategy::DirectSlot {
                target_contract: slot.0.into_legacy(),
                slot: slot.1.into_legacy(),
            });
        }

        // Multiple storage slots accessed - test each one to find the balance slot
        tracing::debug!(
            ?token,
            total = storage_slots.len(),
            "multiple SLOAD operations, testing each one",
        );

        sort_storage_overrides(&mut storage_slots, &holder, self.probing_depth.into());

        // We check slots individually/one at a time instead of all at once because
        // changing unnecessary storage slots could negatively affect the execution (ex.
        // overriding an upgradable proxy contract target)
        for (i, slot) in storage_slots
            .iter()
            .take(self.probing_depth.into())
            .enumerate()
        {
            if let Ok(strategy) = self.verify_slot_is_balance(token, holder, *slot).await {
                tracing::debug!(
                    ?token,
                    ?holder,
                    ?slot,
                    iterations = i + 1,
                    total = storage_slots.len(),
                    "verified balance SLOAD slot via testing",
                );
                return Ok(strategy);
            }
        }

        tracing::debug!(
            "none of the SLOAD slots appear to be the balance slot for token {:?}",
            token
        );

        Err(DetectionError::NotFound)
    }

    /// Extracts storage slots accessed via SLOAD operations from struct logs.
    /// Returns slots in the order they were accessed.
    fn extract_sload_slots(
        &self,
        trace: GethTrace,
        initial_storage_context: Address,
    ) -> Vec<(Address, B256)> {
        let mut storage_context = vec![initial_storage_context];
        let mut slots = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for log in &trace
            .try_into_default_frame()
            .unwrap_or_default()
            .struct_logs
        {
            let stack = log.stack.clone().unwrap_or_default();
            match log.op.as_ref() {
                // CALL or STATICCALL calls another contract, possibly reading data
                // we need to keep track of this contract being called so we know to update the
                // state of this different contract
                "CALL" | "STATICCALL" if stack.len() >= 2 => {
                    // CALL opcode takes the address of the contract to call as second element
                    tracing::trace!("Detected CALL into nested contract");
                    storage_context.push(Address::from_word(stack[stack.len() - 2].into()));
                }
                "DELEGATECALL" if stack.len() >= 2 => {
                    // DELEGATECALL opcode uses the same execution context as the previous contract
                    // but we still need to push it again so the RETURN does not
                    // break
                    storage_context.push(*storage_context.last().unwrap());
                }
                // RETURN is when a contract that was called returns
                "RETURN" => {
                    tracing::trace!("Detected RETURN from nested contract");
                    if storage_context.is_empty() {
                        tracing::debug!(
                            "Too many RETURN opcodes (is there something wrong with the struct \
                             log?)"
                        );
                        break;
                    }
                    storage_context.pop();
                }
                // SLOAD opcode reads from storage
                // The storage key is on top of the stack (last element)
                "SLOAD" if !stack.is_empty() => {
                    tracing::trace!("Detected SLOAD");
                    // Stack grows upward, so the top is the last element
                    let slot = *stack.last().unwrap();

                    // Only add unique slots, preserving order
                    if seen.insert((*storage_context.last().unwrap(), slot)) {
                        slots.push((*storage_context.last().unwrap(), slot.into()));
                    }
                }
                _ => {} // Ignore other opcodes
            }
        }

        slots
    }

    /// Verifies that a storage slot controls the balance by setting it and
    /// checking balanceOf.
    async fn verify_slot_is_balance(
        &self,
        token: Address,
        holder: Address,
        slot: (Address, B256),
    ) -> Result<Strategy, DetectionError> {
        // Use a unique test value to verify this is the balance slot
        let test_balance = U256::from(0x1337_1337_1337_1337_u64);

        // Create state override with the test value in this slot
        let mut slot_value = [0u8; 32];
        test_balance.to_big_endian(&mut slot_value);

        let overrides = hashmap! {
            slot.0.into_legacy() => StateOverride {
                state_diff: Some(hashmap! {
                    slot.1.into_legacy() => H256(slot_value),
                }),
                ..Default::default()
            },
        };

        // Call balanceOf with the override
        let token_contract = ERC20::Instance::new(token, self.web3.alloy.clone());
        let balance = token_contract
            .balanceOf(holder)
            .state(overrides.into_alloy())
            .call()
            .await
            .map_err(|e| {
                DetectionError::Simulation(SimulationError::Other(anyhow::anyhow!(
                    "balanceOf call failed during slot verification: {e}"
                )))
            })?;

        // If the balance matches our test value, we found the right slot
        if balance == test_balance.into_alloy() {
            Ok(Strategy::DirectSlot {
                target_contract: slot.0.into_legacy(),
                slot: slot.1.into_legacy(),
            })
        } else {
            Err(DetectionError::NotFound)
        }
    }
}

/// Contains all the information we need to determine which state override
/// was successful.
struct StrategyHelper {
    /// strategy that was used to compute the state override
    strategy: Strategy,
    /// balance amount the strategy wrote into the storage
    balance: U256,
}

impl StrategyHelper {
    fn new(strategy: Strategy, index: usize) -> Self {
        let index = u8::try_from(index).expect("unreasonable amount of strategies used");
        Self {
            strategy,
            // Use an exact value which isn't too large or too small. This helps
            // not have false positives for cases where the token balances in
            // some other denomination from the actual token balance (such as
            // stETH for example) and not run into issues with overflows.
            // We also make sure that we avoid 0 because `balanceOf()` returns
            // 0 by default so we can't use it to detect successful state overrides.
            balance: U256::from(u64::from_be_bytes([index + 1; 8])),
        }
    }
}

// <https://github.com/OpenZeppelin/openzeppelin-contracts-upgradeable/blob/master/contracts/token/ERC20/ERC20Upgradeable.sol#L43-L44>
const OPEN_ZEPPELIN_ERC20_UPGRADEABLE: &str =
    "52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00";

impl Debug for Detector {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Detector")
            .field("simulator", &format_args!("Arc<dyn CodeSimulating>"))
            .finish()
    }
}

/// An error detecting the balance override strategy for a token.
#[derive(Debug, Error)]
pub enum DetectionError {
    #[error("could not detect a balance override strategy")]
    NotFound,
    #[error(transparent)]
    Simulation(#[from] SimulationError),
}

#[cfg(test)]
mod tests {
    use {super::*, ethrpc::Web3};

    /// Tests that we can detect storage slots by probing the first
    /// n slots or by checking hardcoded known slots.
    /// Set `NODE_URL` environment to a mainnet RPC URL.
    #[ignore]
    #[tokio::test]
    async fn detects_storage_slots_mainnet() {
        use alloy::primitives::address;

        let detector = Detector::new(Web3::new_from_env(), 60);

        let storage = detector
            .detect(
                address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                address!("0000000000000000000000000000000000000001"),
            )
            .await
            .unwrap();
        assert_eq!(
            storage,
            Strategy::SolidityMapping {
                target_contract: addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                map_slot: 3.into()
            }
        );

        let storage = detector
            .detect(
                address!("4956b52ae2ff65d74ca2d61207523288e4528f96"),
                address!("0000000000000000000000000000000000000001"),
            )
            .await
            .unwrap();
        assert_eq!(
            storage,
            Strategy::SolidityMapping {
                target_contract: addr!("4956b52ae2ff65d74ca2d61207523288e4528f96"),
                map_slot: U256::from(OPEN_ZEPPELIN_ERC20_UPGRADEABLE),
            }
        );

        let storage = detector
            .detect(
                address!("0000000000c5dc95539589fbd24be07c6c14eca4"),
                address!("0000000000000000000000000000000000000001"),
            )
            .await
            .unwrap();
        assert_eq!(
            storage,
            Strategy::SoladyMapping {
                target_contract: addr!("0000000000c5dc95539589fbd24be07c6c14eca4")
            }
        );
    }

    /// Tests that we can detect storage slots by probing the first
    /// n slots or by checking hardcoded known slots.
    /// Set `NODE_URL` environment to an arbitrum RPC URL.
    #[ignore]
    #[tokio::test]
    async fn detects_storage_slots_arbitrum() {
        use alloy::primitives::address;

        let detector = Detector::new(Web3::new_from_env(), 60);

        // all bridged tokens on arbitrum require a ton of probing
        let storage = detector
            .detect(
                address!("ff970a61a04b1ca14834a43f5de4533ebddb5cc8"),
                address!("0000000000000000000000000000000000000001"),
            )
            .await
            .unwrap();
        assert_eq!(
            storage,
            Strategy::SolidityMapping {
                target_contract: addr!("ff970a61a04b1ca14834a43f5de4533ebddb5cc8"),
                map_slot: 51.into()
            }
        );
    }

    #[test]
    fn test_sort_storage_overrides_reverses_order() {
        use alloy::primitives::{address, b256};

        let contract = address!("0000000000000000000000000000000000000002");
        let contract2 = address!("0000000000000000000000000000000000000002");
        let slot1 = b256!("0000000000000000000000000000000000000000000000000000000000000001");
        let slot2 = b256!("0000000000000000000000000000000000000000000000000000000000000002");
        let slot3 = b256!("0000000000000000000000000000000000000000000000000000000000000003");
        let mut slots = vec![
            (contract, slot1),
            (contract2, slot3),
            (contract, slot2),
            (contract, slot3),
            (contract2, slot1),
        ];
        let holder = address!("0000000000000000000000000000000000000001");

        // These slots don't match any heuristic, so should just be reversed
        sort_storage_overrides(&mut slots, &holder, 5);

        assert_eq!(
            slots,
            vec![
                (contract2, slot1),
                (contract, slot3),
                (contract, slot2),
                (contract2, slot3),
                (contract, slot1),
            ]
        );
    }

    #[test]
    fn test_sort_storage_overrides_with_heuristic_slots_stable() {
        use alloy::primitives::{address, b256};

        let contract = address!("0000000000000000000000000000000000000002");
        let holder = address!("1111111111111111111111111111111111111111");

        // Calculate what slot index 0 would hash to for this holder
        let mut buf = [0u8; 64];
        buf[12..32].copy_from_slice(holder.as_slice());
        U256::from(0).to_big_endian(&mut buf[32..64]);
        let heuristic_slot1 = B256::from(signing::keccak256(&buf));
        U256::from(5).to_big_endian(&mut buf[32..64]);
        let heuristic_slot2 = B256::from(signing::keccak256(&buf));
        U256::from(10).to_big_endian(&mut buf[32..64]);
        let heuristic_slot3 = B256::from(signing::keccak256(&buf));

        // Create non-heuristic slots
        let slot1 = b256!("00000000000000000000000000000000000000000000000000000000000003e7");
        let slot2 = b256!("00000000000000000000000000000000000000000000000000000000000003e6");
        let slot3 = b256!("00000000000000000000000000000000000000000000000000000000000003e3");

        let mut slots = vec![
            (contract, slot1),
            (contract, slot3),
            (contract, heuristic_slot2),
            (contract, heuristic_slot3),
            (contract, slot2),
            (contract, heuristic_slot1),
        ];

        sort_storage_overrides(&mut slots, &holder, 100);

        // After reverse: [heuristic, non_heuristic]
        // After sort: non-heuristic slots come before heuristic slots due to false <
        // true And the reversed order should be preserved
        // So: [non_heuristic, heuristic]
        assert_eq!(
            slots,
            vec![
                (contract, heuristic_slot1),
                (contract, heuristic_slot3),
                (contract, heuristic_slot2),
                (contract, slot2),
                (contract, slot3),
                (contract, slot1),
            ]
        );
    }

    #[test]
    fn test_sort_storage_overrides_zero_heuristic_depth() {
        use alloy::primitives::{address, b256};

        let contract = address!("0000000000000000000000000000000000000002");
        let holder = address!("5555555555555555555555555555555555555555");

        let slot1 = b256!("0000000000000000000000000000000000000000000000000000000000000001");
        let slot2 = b256!("0000000000000000000000000000000000000000000000000000000000000002");
        let slot3 = b256!("0000000000000000000000000000000000000000000000000000000000000003");

        let mut buf = [0u8; 64];
        buf[12..32].copy_from_slice(holder.as_slice());
        U256::from(0).to_big_endian(&mut buf[32..64]);
        let heuristic_slot = B256::from(signing::keccak256(&buf));

        let mut slots = vec![
            (contract, slot1),
            (contract, slot2),
            (contract, heuristic_slot),
            (contract, slot3),
        ];

        sort_storage_overrides(&mut slots, &holder, 0);

        // With depth 0, no heuristic slots are created, so just reversed
        assert_eq!(
            slots,
            vec![
                (contract, slot3),
                (contract, heuristic_slot),
                (contract, slot2),
                (contract, slot1),
            ]
        );
    }
}
