use {
    super::Strategy,
    crate::{StrategyExt, aave},
    alloy_eips::BlockId,
    alloy_primitives::{Address, B256, TxKind, U256, keccak256},
    alloy_provider::ext::DebugApi,
    alloy_rpc_types::{
        TransactionInput,
        TransactionRequest,
        trace::geth::{GethDebugTracingCallOptions, GethTrace},
    },
    alloy_sol_types::SolCall,
    alloy_transport::{RpcError, TransportErrorKind},
    contracts::ERC20,
    std::{
        collections::HashMap,
        fmt::{self, Debug, Formatter},
        sync::Arc,
        time::Duration,
    },
    thiserror::Error,
};

pub const DEFAULT_VERIFICATION_TIMEOUT: Duration = Duration::from_secs(1);

// These are the solady magic bytes for user balances, with padding
// https://github.com/Vectorized/solady/blob/main/src/tokens/ERC20.sol#L81
const SOLADY_MAGIC_BYTES: &[u8] = &[
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x87, 0xa2, 0x11, 0xa2,
];

/// Error that occurs when verifying a balance override strategy.
#[derive(Debug, Error)]
pub enum SimulationError {
    #[error("simulation reverted {0:?}")]
    Revert(Option<String>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A heuristic balance override detector based on `eth_call` simulations.
///
/// This has the exact same node requirements as trade verification.
#[derive(Clone)]
pub struct Detector(Arc<Inner>);

pub struct Inner {
    probing_depth: u8,
    web3: ethrpc::Web3,
    verification_timeout: Duration,
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
///
/// Note: this function does not emit `AaveV3AToken` candidates. Aave v3 is
/// handled by the fast-path in `detect()` which tries the canonical
/// `_userState` slot directly; the fallback here is for non-Aave tokens only.
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
        let slot_hash = keccak256(buf);
        solidity_mapping_slot_to_index.insert(slot_hash, i);
    }

    buf[0..20].copy_from_slice(holder.as_slice());
    buf[20..32].copy_from_slice(SOLADY_MAGIC_BYTES);
    let solady_slot = keccak256(&buf[0..32]);

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
    pub fn new(web3: ethrpc::Web3, probing_depth: u8, verification_timeout: Duration) -> Self {
        Self(Arc::new(Inner {
            web3,
            probing_depth,
            verification_timeout,
        }))
    }

    /// The `Web3` handle shared with whoever constructed the detector; also
    /// used by `BalanceOverrides` for strategies that need on-chain reads at
    /// override-resolution time (e.g. `AaveV3AToken`).
    pub fn web3(&self) -> &ethrpc::Web3 {
        &self.web3
    }

    /// Detects the balance storage slot for `token`.
    ///
    /// Takes a fast path for Aave v3 aTokens: a cheap two-call probe, plus
    /// a single verification using the canonical `_userState` slot — no
    /// `debug_traceCall` needed on the happy path. Everything else falls
    /// through to the generic SLOAD-trace detection.
    pub async fn detect(
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
    ///
    /// For the `AaveV3AToken` variant we allow `balanceOf` to be off by one
    /// wei, since Aave's ray rounding can slightly differ from our locally
    /// computed scaled value; for every other variant the check is still an
    /// exact equality.
    async fn verify_strategy(
        &self,
        token: Address,
        holder: Address,
        strategy: &Strategy,
    ) -> Result<(), DetectionError<TransportErrorKind>> {
        // Use a unique test value to verify this strategy works
        let test_balance = U256::from(0x1337_1337_1337_1337_u64);

        let overrides = strategy
            .state_override(Some(&self.web3), &holder, &test_balance)
            .await;
        if overrides.is_empty() {
            return Err(DetectionError::NotFound);
        }

        // Call balanceOf with the override
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
        alloy_primitives::{B256, U256, address, b256},
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
        let detector = Detector::new(Web3::new_from_env(), 60, DEFAULT_VERIFICATION_TIMEOUT);

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
        let detector = Detector::new(Web3::new_from_env(), 60, DEFAULT_VERIFICATION_TIMEOUT);

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
        let heuristic_slot1 = keccak256(buf);
        buf[32..64].copy_from_slice(&U256::from(5).to_be_bytes::<32>());
        let heuristic_slot2 = keccak256(buf);
        buf[0..20].copy_from_slice(holder.as_slice());
        buf[20..32].copy_from_slice(SOLADY_MAGIC_BYTES);
        let heuristic_slot3 = keccak256(&buf[0..32]);

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
        let heuristic_slot = keccak256(buf);

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
