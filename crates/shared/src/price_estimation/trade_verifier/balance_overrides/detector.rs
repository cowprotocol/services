use {
    super::Strategy,
    crate::tenderly_api::SimulationError,
    alloy::{
        eips::BlockId,
        primitives::{Address, B256, TxKind, U256},
        providers::ext::DebugApi,
        rpc::types::{
            TransactionInput,
            TransactionRequest,
            trace::geth::{GethDebugTracingCallOptions, GethTrace},
        },
        sol_types::SolCall,
        transports::{RpcError, TransportErrorKind},
    },
    contracts::alloy::ERC20,
    std::{
        collections::HashMap,
        fmt::{self, Debug, Formatter},
        sync::Arc,
    },
    thiserror::Error,
};

// These are the solady magic bytes for user balances, with padding
// https://github.com/Vectorized/solady/blob/main/src/tokens/ERC20.sol#L81
const SOLADY_MAGIC_BYTES: &[u8] = &[
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x87, 0xa2, 0x11, 0xa2,
];

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

/// Used by Detector.detect() when there are multiple slots to increase the
/// chances we find the correct storage slot quickly. Returns a list of
/// strategies to try. Strategies employed:
/// 1. Reverse because the most recently accessed storage slots are most likely
///    to be the return value of `balanceOf`
/// 2. Use the Solidity mapping hashes as a heuristic and prioritize scanning
///    those storage slots first - these return `SolidityMapping` strategy
/// 3. For non-heuristic slots, return `DirectSlot` strategy
fn create_strategies_from_slots(
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
        let slot_hash = alloy::primitives::keccak256(buf);
        solidity_mapping_slot_to_index.insert(slot_hash, i);
    }

    buf[0..20].copy_from_slice(holder.as_slice());
    buf[20..32].copy_from_slice(SOLADY_MAGIC_BYTES);
    let solady_slot = alloy::primitives::keccak256(&buf[0..32]);

    // We separate heuristic and non-heuristic in a single pass,
    // iterating in reverse so "most recently accessed" come first.
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

    // Heuristics first, then non-heuristics, each already in
    // "most recent first" order due to .rev() above.
    heuristic_strategies.extend(fallback_strategies);
    heuristic_strategies
}

impl Detector {
    /// Creates a new balance override detector.
    pub fn new(web3: ethrpc::Web3, probing_depth: u8) -> Self {
        Self(Arc::new(Inner {
            web3,
            probing_depth,
        }))
    }

    /// Detects the balance storage slot using debug_traceCall, similar to
    /// Foundry's `deal`. This traces a balanceOf call and finds which
    /// storage slot is accessed. If more than one slot is accessed, we reverse
    /// by order accessed, sort by if the slot was one that would normally
    /// be seen in solidity mapping, and test one by one to see if the
    /// override is effective.
    pub async fn detect(
        &self,
        token: Address,
        holder: Address,
    ) -> Result<Strategy, DetectionError<TransportErrorKind>> {
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
            .map_err(|err| {
                tracing::debug!(?token, ?err, "debug_traceCall not supported for token");
                DetectionError::Rpc(err)
            })?;

        // Extract storage slots accessed via SLOAD operations
        let storage_slots = self.extract_sload_slots(trace, token);

        if storage_slots.is_empty() {
            tracing::debug!("no SLOAD operations found in trace for token {:?}", token);
            return Err(DetectionError::NotFound);
        }

        let strategies =
            create_strategies_from_slots(&storage_slots, &holder, self.probing_depth.into());

        if strategies.len() == 1 {
            let slot = storage_slots[0];
            tracing::debug!(
                storage_context = ?slot.0,
                slot = ?slot.1,
                ?slot,
                iterations = 0,
                "detected balance slot via trace (single SLOAD) for token",
            );

            return Ok(strategies[0].clone());
        }

        // Multiple storage slots accessed - test each one to find the balance slot
        tracing::debug!(
            ?token,
            total = storage_slots.len(),
            "multiple SLOAD operations, testing each one",
        );

        // We check slots individually/one at a time instead of all at once because
        // changing unnecessary storage slots could negatively affect the execution (ex.
        // overriding an upgradable proxy contract target)
        for (i, strategy) in strategies
            .iter()
            .take(self.probing_depth.into())
            .enumerate()
        {
            if self.verify_strategy(token, holder, strategy).await.is_ok() {
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
                    if let Some(current_storage) = storage_context.last() {
                        tracing::trace!(?stack, "Detected SLOAD");
                        // Stack grows upward, so the top is the last element
                        let slot = *stack.last().unwrap();

                        // Only add unique slots, preserving order
                        if seen.insert((*current_storage, slot)) {
                            slots.push((*current_storage, slot.into()));
                        }
                    } else {
                        tracing::debug!(
                            ?stack,
                            "SLOAD called when not in a call context (is something wrong with the \
                             struct log?)"
                        );
                        break;
                    }
                }
                _ => {} // Ignore other opcodes
            }
        }

        slots
    }

    /// Verifies that a strategy correctly controls the balance by applying it
    /// and checking balanceOf.
    async fn verify_strategy(
        &self,
        token: Address,
        holder: Address,
        strategy: &Strategy,
    ) -> Result<(), DetectionError<TransportErrorKind>> {
        // Use a unique test value to verify this strategy works
        let test_balance = alloy::primitives::U256::from(0x1337_1337_1337_1337_u64);

        // Create state override using the strategy
        let overrides = strategy.state_override(&holder, &test_balance);

        // Call balanceOf with the override
        let token_contract = ERC20::Instance::new(token, self.web3.alloy.clone());
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

        // If the balance matches our test value, the strategy works
        if balance == test_balance {
            Ok(())
        } else {
            Err(DetectionError::NotFound)
        }
    }
}

impl Debug for Detector {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Detector")
            .field("simulator", &format_args!("Arc<dyn CodeSimulating>"))
            .finish()
    }
}

/// An error detecting the balance override strategy for a token.
#[derive(Debug, Error)]
pub enum DetectionError<E> {
    #[error("could not detect a balance override strategy")]
    NotFound,
    #[error("error returned by the RPC server")]
    Rpc(#[from] RpcError<E>),
    #[error(transparent)]
    Simulation(#[from] SimulationError),
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{B256, address, b256},
        ethrpc::Web3,
    };

    // <https://github.com/OpenZeppelin/openzeppelin-contracts-upgradeable/blob/master/contracts/token/ERC20/ERC20Upgradeable.sol#L43-L44>
    const OPEN_ZEPPELIN_ERC20_UPGRADEABLE: B256 =
        b256!("52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00");

    /// Tests that we can detect storage slots by probing the first
    /// n slots or by checking hardcoded known slots.
    /// Set `NODE_URL` environment to a mainnet RPC URL.
    #[ignore]
    #[tokio::test]
    async fn detects_storage_slots_mainnet() {
        let detector = Detector::new(Web3::new_from_env(), 60);

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

    /// Tests that we can detect storage slots by probing the first
    /// n slots or by checking hardcoded known slots.
    /// Set `NODE_URL` environment to an arbitrum RPC URL.
    #[ignore]
    #[tokio::test]
    async fn detects_storage_slots_arbitrum() {
        let detector = Detector::new(Web3::new_from_env(), 60);

        // all bridged tokens on arbitrum require a ton of probing
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

        // These slots don't match any heuristic, so should just be reversed and return
        // DirectSlot
        let strategies = create_strategies_from_slots(&slots, &holder, 5);

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

        // Calculate what slot index 0 would hash to for this holder
        let mut buf = [0u8; 64];
        buf[12..32].copy_from_slice(holder.as_slice());
        buf[32..64].copy_from_slice(&U256::ZERO.to_be_bytes::<32>());
        let heuristic_slot1 = alloy::primitives::keccak256(buf);
        buf[32..64].copy_from_slice(&U256::from(5).to_be_bytes::<32>());
        let heuristic_slot2 = alloy::primitives::keccak256(buf);
        buf[0..20].copy_from_slice(holder.as_slice());
        buf[20..32].copy_from_slice(SOLADY_MAGIC_BYTES);
        let heuristic_slot3 = alloy::primitives::keccak256(&buf[0..32]);

        // Create non-heuristic slots
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

        let strategies = create_strategies_from_slots(&slots, &holder, 100);

        // After reverse: [heuristic, non_heuristic]
        // After sort: non-heuristic slots come before heuristic slots due to false <
        // true And the reversed order should be preserved
        // So: [heuristic (as SolidityMapping), non_heuristic (as DirectSlot)]
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
        let heuristic_slot = alloy::primitives::keccak256(buf);

        let slots = vec![
            (contract, slot1),
            (contract, slot2),
            (contract, heuristic_slot),
            (contract, slot3),
        ];

        let strategies = create_strategies_from_slots(&slots, &holder, 0);

        // With depth 0, no heuristic slots are created, so all become DirectSlot and
        // just reversed
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
}
